'use strict';

const {test} = require('node:test');
const assert = require('node:assert/strict');
const automerge = require('./automerge.cjs');

function fixture() {
  const bot = {login: 'dependabot[bot]', type: 'Bot'};
  const repo = {full_name: 'example/project'};
  const pr = {
    number: 7, state: 'open', draft: false, user: bot, commits: 1,
    base: {ref: 'main', repo}, head: {sha: 'a'.repeat(40), repo},
    auto_merge: null,
  };
  const f = {
    eventName: 'pull_request_target', event: structuredClone(pr), pr,
    dependencies: [{updateType: 'version-update:semver-patch'}], allowMinor: 'false',
    commits: [{sha: pr.head.sha, author: structuredClone(bot), commit: {verification: {verified: true}}}],
    settings: {autoMergeAllowed: true, squashMergeAllowed: true, ref: {
      branchProtectionRule: {requiresStatusChecks: true, requiredStatusCheckContexts: ['CI']},
    }},
    rules: [], outputs: {}, calls: [],
  };
  f.run = () => automerge({
    context: {
      eventName: f.eventName, repo: {owner: 'example', repo: 'project'},
      payload: {pull_request: f.event, repository: {default_branch: 'main'}},
    },
    core: {notice() {}, setOutput: (key, value) => { f.outputs[key] = value; }},
    github: {
      rest: {pulls: {
        get: async params => { f.calls.push(['pr', params]); return {data: f.pr}; },
        listCommits: 'commits',
      }},
      paginate: async (route, params) => {
        f.calls.push([route, params]);
        if (f.apiError) throw new Error('API unavailable');
        return route === 'commits' ? f.commits : f.rules;
      },
      graphql: async (query, params) => {
        f.calls.push(['settings', params]);
        return {repository: f.settings};
      },
    },
    exec: {exec: async (command, args) => {
      f.calls.push(['exec', command, args]);
      if (f.execError) throw new Error('Head changed before merge');
    }},
  }, {
    allowMinor: f.allowMinor,
    metadata: f.rawMetadata ?? JSON.stringify(f.dependencies),
  });
  return f;
}

test('patch requests GitHub auto-merge with the evaluated head and no bypass', async () => {
  const f = fixture();
  await f.run();
  assert.equal(f.outputs.decision, 'requested');
  assert.deepEqual(f.calls.at(-1), ['exec', 'gh', [
    'pr', 'merge', '7', '--repo', 'example/project', '--auto', '--squash',
    '--match-head-commit', 'a'.repeat(40),
  ]]);
});

test('minor requires explicit opt-in, including grouped updates', async () => {
  const f = fixture();
  f.dependencies.push({updateType: 'version-update:semver-minor'});
  await f.run();
  assert.equal(f.outputs.decision, 'skipped');
  assert.deepEqual(f.calls, []);
  f.allowMinor = 'true';
  await f.run();
  assert.equal(f.outputs.decision, 'requested');
});

for (const type of ['version-update:semver-major', '', 'unknown', undefined]) {
  test(`mixed group with ${String(type)} never qualifies, even with minor opt-in`, async () => {
    const f = fixture();
    f.allowMinor = 'true';
    f.dependencies.push({updateType: type});
    await f.run();
    assert.equal(f.outputs.decision, 'skipped');
    assert.deepEqual(f.calls, []);
  });
}

for (const raw of ['', 'not JSON', '{}', '[]', 'null', '[null]']) {
  test(`missing or malformed metadata ${JSON.stringify(raw)} stays for review`, async () => {
    const f = fixture();
    f.rawMetadata = raw;
    await f.run();
    assert.equal(f.outputs.decision, 'skipped');
    assert.deepEqual(f.calls, []);
  });
}

const ineligible = {
  'unprivileged event': f => { f.eventName = 'pull_request'; },
  'non-bot author': f => { f.event.user.login = 'someone'; },
  'wrong account type': f => { f.event.user.type = 'User'; },
  'fork': f => { f.event.head.repo.full_name = 'someone/project'; },
  'other base repository': f => { f.event.base.repo.full_name = 'someone/project'; },
  'other target branch': f => { f.event.base.ref = 'release'; },
  'draft': f => { f.event.draft = true; },
  'closed PR': f => { f.event.state = 'closed'; },
  'deleted head repository': f => { f.event.head.repo = null; },
  'missing pull request': f => { f.event = undefined; },
};
for (const [name, change] of Object.entries(ineligible)) {
  test(`${name} cannot request a merge`, async () => {
    const f = fixture();
    change(f);
    await f.run();
    assert.equal(f.outputs.decision, 'skipped');
    assert.deepEqual(f.calls, []);
  });
}

for (const change of [
  f => { f.pr.head.sha = 'b'.repeat(40); },
  f => { f.pr.base.ref = 'release'; },
  f => { f.pr.draft = true; },
  f => { f.pr.state = 'closed'; },
]) {
  test('changed live PR is rejected despite an eligible event', async () => {
    const f = fixture();
    change(f);
    await f.run();
    assert.equal(f.outputs.decision, 'skipped');
    assert.equal(f.calls.length, 1);
  });
}

for (const change of [
  f => { f.commits[0].author = null; },
  f => { f.commits[0].author.login = 'someone'; },
  f => { f.commits[0].commit.verification.verified = false; },
  f => { f.commits = []; },
  f => { f.pr.commits = 2; },
  f => { f.commits[0].sha = 'b'.repeat(40); },
]) {
  test('unverified, incomplete or stale commit history cannot merge', async () => {
    const f = fixture();
    change(f);
    await f.run();
    assert.equal(f.outputs.decision, 'skipped');
    assert.equal(f.calls.some(call => call[0] === 'exec'), false);
  });
}

test('a human commit appended to a genuine bot PR cannot merge', async () => {
  const f = fixture();
  const human = structuredClone(f.commits[0]);
  human.author.login = 'someone';
  f.commits.unshift(human);
  f.pr.commits = 2;
  await f.run();
  assert.equal(f.outputs.decision, 'skipped');
});

test('active rulesets work without classic branch protection', async () => {
  const f = fixture();
  f.settings.ref.branchProtectionRule = null;
  f.rules = [{type: 'required_status_checks', parameters: {required_status_checks: [{context: 'CI'}]}}];
  await f.run();
  assert.equal(f.outputs.decision, 'requested');
  assert.ok(f.calls.some(call => call[0].includes('/rules/branches/')));
});

test('unprotected branches and empty required-check rules fail with setup instructions', async () => {
  for (const rules of [[], [{type: 'required_status_checks', parameters: {required_status_checks: []}}]]) {
    const f = fixture();
    f.settings.ref.branchProtectionRule.requiredStatusCheckContexts = [];
    f.rules = rules;
    await assert.rejects(f.run(), /at least one required CI check/);
    assert.equal(f.calls.some(call => call[0] === 'exec'), false);
  }
});

test('unavailable API fails closed', async () => {
  const f = fixture();
  f.apiError = true;
  await assert.rejects(f.run(), /API unavailable/);
  assert.equal(f.calls.some(call => call[0] === 'exec'), false);
});

for (const setting of ['autoMergeAllowed', 'squashMergeAllowed']) {
  test(`disabled ${setting} reports the missing setting`, async () => {
    const f = fixture();
    f.settings[setting] = false;
    await assert.rejects(f.run(), /Enable Allow auto-merge/);
    assert.equal(f.calls.some(call => call[0] === 'exec'), false);
  });
}

test('rerunning an already enabled PR does not request another merge', async () => {
  const f = fixture();
  f.pr.auto_merge = {};
  await f.run();
  assert.equal(f.outputs.decision, 'already-enabled');
  assert.equal(f.calls.some(call => call[0] === 'exec'), false);
});

test('CLI failure, including a concurrent head change, is not reported as success', async () => {
  const f = fixture();
  f.execError = true;
  await assert.rejects(f.run(), /Head changed/);
  assert.equal(f.outputs.decision, undefined);
});

test('invalid policy input fails before any API call', async () => {
  const f = fixture();
  f.allowMinor = 'yes';
  await assert.rejects(f.run(), /allow-minor must be true or false/);
  assert.deepEqual(f.calls, []);
});
