'use strict';

// This runs from the pinned action, never from the pull request's checkout.
module.exports = async function automerge({github, context, core, exec}, options) {
  const skip = reason => {
    core.notice(reason);
    core.setOutput('result', 'skipped');
  };
  if (!['true', 'false'].includes(options.allowMinor)) {
    throw new Error('allow-minor must be true or false');
  }
  const event = context.payload.pull_request;
  const repository = `${context.repo.owner}/${context.repo.repo}`;
  const defaultBranch = context.payload.repository?.default_branch;
  const eligible = pr => pr?.user?.login === 'dependabot[bot]' &&
    pr.user.type === 'Bot' && pr.state === 'open' && !pr.draft &&
    pr.head?.repo?.full_name === repository && pr.base?.repo?.full_name === repository &&
    pr.base.ref === defaultBranch;
  if (context.eventName !== 'pull_request_target' || !eligible(event)) {
    return skip('Only open, same-repository Dependabot PRs into the default branch are eligible.');
  }
  let dependencies;
  try {
    dependencies = JSON.parse(options.metadata);
  } catch {
    return skip('Verified dependency metadata is unavailable.');
  }
  const allowed = ['version-update:semver-patch'];
  if (options.allowMinor === 'true') allowed.push('version-update:semver-minor');
  if (!Array.isArray(dependencies) || !dependencies.length ||
      !dependencies.every(dep => dep && allowed.includes(dep.updateType))) {
    return skip('Every dependency in the PR must have an allowed update type.');
  }

  const params = {...context.repo, pull_number: event.number};
  const {data: pr} = await github.rest.pulls.get(params);
  if (!eligible(pr) || pr.head.sha !== event.head.sha) {
    return skip('The pull request changed; a fresh event must evaluate it.');
  }
  // fetch-metadata verifies the first commit. Check the complete history too,
  // so an appended human commit cannot inherit the bot's eligibility.
  const commits = await github.paginate(github.rest.pulls.listCommits, {...params, per_page: 100});
  if (!commits.length || commits.length !== pr.commits || commits.at(-1).sha !== pr.head.sha ||
      !commits.every(c => c.author?.login === 'dependabot[bot]' &&
        c.author.type === 'Bot' && c.commit?.verification?.verified === true)) {
    return skip('All commits must be verified Dependabot commits at the evaluated head.');
  }

  const {repository: settings} = await github.graphql(`
    query($owner: String!, $repo: String!, $branch: String!) {
      repository(owner: $owner, name: $repo) {
        autoMergeAllowed
        squashMergeAllowed
        ref(qualifiedName: $branch) {
          branchProtectionRule { requiresStatusChecks requiredStatusCheckContexts }
        }
      }
    }`, {...context.repo, branch: `refs/heads/${pr.base.ref}`});
  const protection = settings?.ref?.branchProtectionRule;
  let requiresChecks = protection?.requiresStatusChecks === true &&
    protection.requiredStatusCheckContexts?.length > 0;
  if (!requiresChecks) {
    const rules = await github.paginate('GET /repos/{owner}/{repo}/rules/branches/{branch}', {
      ...context.repo, branch: pr.base.ref, per_page: 100,
    });
    requiresChecks = rules.some(rule => rule.type === 'required_status_checks' &&
      rule.parameters?.required_status_checks?.length > 0);
  }
  if (!requiresChecks) {
    throw new Error('Configure at least one required CI check on the default branch before enabling dependency auto-merge.');
  }
  if (!settings.autoMergeAllowed || !settings.squashMergeAllowed) {
    throw new Error('Enable Allow auto-merge and Allow squash merging in the consuming repository settings.');
  }
  if (pr.auto_merge) {
    core.setOutput('result', 'already-enabled');
    return;
  }

  // GitHub enforces required checks/reviews, and the CLI refuses a stale head.
  // No approval or administrator bypass is requested.
  await exec.exec('gh', [
    'pr', 'merge', String(pr.number), '--repo', repository,
    '--auto', '--squash', '--match-head-commit', pr.head.sha,
  ]);
  core.setOutput('result', 'requested');
  core.notice('Requested dependency merge through GitHub required checks and reviews.');
};
