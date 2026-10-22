"""Exercise private DNS, proxies and CA trust without model calls or credentials."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def run(*args, check=True):
    return subprocess.run(args, check=check, text=True, capture_output=True)


image = os.environ.get("MAINTAINER_TEST_IMAGE", "repo-maintainer:test")
network = f"maintainer-network-{os.getpid()}"
server = f"maintainer-reference-{os.getpid()}"
url = "https://reference.internal/reference"
with tempfile.TemporaryDirectory() as temporary:
    root = Path(temporary)
    root.chmod(0o755)
    run("openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
        "-keyout", str(root / "ca.key"), "-out", str(root / "ca.crt"),
        "-subj", "/CN=Maintainer Network Test CA")
    run("openssl", "req", "-new", "-newkey", "rsa:2048", "-nodes",
        "-keyout", str(root / "server.key"), "-out", str(root / "server.csr"),
        "-subj", "/CN=reference.internal")
    (root / "extensions").write_text("subjectAltName=DNS:reference.internal\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\n")
    run("openssl", "x509", "-req", "-in", str(root / "server.csr"),
        "-CA", str(root / "ca.crt"), "-CAkey", str(root / "ca.key"), "-CAcreateserial",
        "-out", str(root / "server.crt"), "-days", "1", "-extfile", str(root / "extensions"))
    # Keep the normal public roots as well as the fixture's private CA.
    (root / "ca-bundle.pem").write_bytes(Path("/etc/ssl/certs/ca-certificates.crt").read_bytes() + (root / "ca.crt").read_bytes())
    check_code = f"import urllib.request; assert b'private service' in urllib.request.urlopen({url!r}).read()"
    (root / ".maintainer.yml").write_text(json.dumps({
        "version": 2, "agent": {"allow_paid": True}, "sources": [url],
        "validation": {"profile": "custom", "checks": [{
            "name": "private-service", "command": ["python3", "-c", check_code]
        }]}
    }))
    (root / "server.py").write_text(r'''
import http.server, select, socket, ssl, threading
class Reference(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = ("<main>A private service reference for network verification. " * 12 + "</main>").encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *args): pass
class Proxy(http.server.BaseHTTPRequestHandler):
    def do_CONNECT(self):
        print("CONNECT " + self.path, flush=True)
        host, port = self.path.rsplit(":", 1)
        remote = socket.create_connection((host, int(port)), timeout=10)
        self.send_response(200); self.end_headers()
        try:
            while True:
                readable, _, _ = select.select([self.connection, remote], [], [], 10)
                if not readable: break
                for stream in readable:
                    data = stream.recv(65536)
                    if not data: return
                    (remote if stream is self.connection else self.connection).sendall(data)
        finally: remote.close()
    def log_message(self, *args): pass
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain("/fixtures/server.crt", "/fixtures/server.key")
reference = http.server.ThreadingHTTPServer(("0.0.0.0", 443), Reference)
reference.socket = tls.wrap_socket(reference.socket, server_side=True)
threading.Thread(target=reference.serve_forever, daemon=True).start()
http.server.ThreadingHTTPServer(("0.0.0.0", 8080), Proxy).serve_forever()
''')
    run("docker", "network", "create", network)
    try:
        run("docker", "run", "--detach", "--rm", "--name", server, "--network", network,
            "--network-alias", "reference.internal", "--network-alias", "proxy.internal",
            "--mount", f"type=bind,source={root},target=/fixtures,readonly",
            "python:3.13-slim", "python3", "/fixtures/server.py")
        for _ in range(30):
            ready = run("docker", "exec", server, "python3", "-c",
                        "import socket; socket.create_connection(('localhost',8080),1).close()", check=False)
            if ready.returncode == 0:
                break
            time.sleep(1)
        else:
            raise RuntimeError("Private network fixture did not start")

        def worker(arguments, *, trust=True, bypass=False, entrypoint="repo-maintainer"):
            command = ["docker", "run", "--rm", "--network", network,
                       "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
                       "--tmpfs", "/tmp:mode=1777",
                       "--tmpfs", "/home/maintainer:uid=10001,gid=10001,mode=0700",
                       "--mount", f"type=bind,source={root},target=/fixtures,readonly",
                       "--env", "MAINTENANCE_CODEX_SANDBOX_DEFAULT=danger-full-access",
                       "--env", "HTTPS_PROXY=http://proxy.internal:8080",
                       "--env", "https_proxy=http://proxy.internal:8080",
                       "--env", "NO_PROXY=" + ("reference.internal" if bypass else ""),
                       "--env", "no_proxy=" + ("reference.internal" if bypass else "")]
            if trust:
                command.extend(["--env", "SSL_CERT_FILE=/fixtures/ca-bundle.pem"])
            return run(*command, "--entrypoint", entrypoint, image, *arguments, check=False)

        reference_args = ["reference", "--repo", "/fixtures", "--url", url]
        untrusted = worker(reference_args, trust=False)
        assert untrusted.returncode != 0, "Untrusted certificates must be rejected"
        reference = worker(reference_args)
        assert reference.returncode == 0, reference.stderr
        assert "private service" in reference.stdout
        checked = worker(["validate", "--repo", "/fixtures"])
        assert checked.returncode == 0, checked.stderr
        before = run("docker", "logs", server).stdout.count("CONNECT ")
        assert before >= 2, "Both native HTTP and configured checks must use the proxy"
        bypass = worker(reference_args, bypass=True)
        assert bypass.returncode == 0, bypass.stderr
        after = run("docker", "logs", server).stdout.count("CONNECT ")
        assert before == after, "NO_PROXY must preserve direct private DNS access"
        sandbox = worker([
            "-c", 'sandbox_mode="danger-full-access"',
            "-c", "sandbox_workspace_write.network_access=true", "sandbox", "--",
            "python3", "-c", check_code,
        ], entrypoint="codex")
        assert sandbox.returncode == 0, sandbox.stderr
        print("Private DNS, proxy use/bypass, TLS verification, child checks and Codex container networking passed.")
    finally:
        run("docker", "rm", "--force", server, check=False)
        run("docker", "network", "rm", network, check=False)
