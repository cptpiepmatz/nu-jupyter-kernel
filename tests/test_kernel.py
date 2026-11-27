import json
import subprocess
from pathlib import Path

import pytest
import tomllib
from jupyter_client.blocking.client import BlockingKernelClient
from jupyter_client.kernelspec import KernelSpecManager
from jupyter_client.manager import KernelManager


TIMEOUT = 10


@pytest.fixture(scope="session")
def kernel_binary():
    build = subprocess.run(
        ["cargo", "build", "--message-format=json"],
        cwd=Path(__file__).resolve().parents[1],
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        check=True,
    )
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if (
            message.get("reason") == "compiler-artifact"
            and message["target"]["name"] == "nu-jupyter-kernel"
            and "bin" in message["target"]["kind"]
            and message.get("executable")
        ):
            return message["executable"]
    pytest.fail("Cargo did not report a nu-jupyter-kernel executable")


@pytest.fixture
def kernel(tmp_path: Path, kernel_binary: str):
    spec_dir = tmp_path / "nu"
    spec_dir.mkdir()
    (spec_dir / "kernel.json").write_text(json.dumps({
        "argv": [kernel_binary, "start", "{connection_file}"],
        "display_name": "Nushell",
        "language": "nushell",
        "interrupt_mode": "message",
    }), encoding="utf-8")
    km = KernelManager(
        kernel_name="nu",
        kernel_spec_manager=KernelSpecManager(kernel_dirs=[str(tmp_path)]),
    )
    km.start_kernel()
    try:
        yield km.client()
    finally:
        km.shutdown_kernel()


def ok(client: BlockingKernelClient, code: str) -> list[dict]:
    # wait until client is ready, then send some code
    client.wait_for_ready(timeout=TIMEOUT)
    client.execute(code)

    # kernel should instantly reply with busy message
    busy_status = client.get_iopub_msg(timeout=TIMEOUT)
    assert busy_status["content"]["execution_state"] == "busy"

    # check on the iopub channel until we receive an idle message
    contents = []
    while True:
        iopub_reply = client.get_iopub_msg(timeout=TIMEOUT)
        if iopub_reply["content"].get("execution_state") == "idle":
            break
        contents.append(iopub_reply["content"])

    # we should get a ok on the shell channel
    shell_reply = client.get_shell_msg(timeout=TIMEOUT)
    assert shell_reply["content"]["status"] == "ok"

    return contents


def test_kernel_info(kernel: BlockingKernelClient):
    kernel.wait_for_ready(timeout=TIMEOUT)
    kernel.kernel_info()

    # control channel not used in jupyter_client
    # control_kernel_info = kernel.get_control_msg(timeout=TIMEOUT)
    shell_kernel_info = kernel.get_shell_msg(timeout=TIMEOUT)

    # assert control_kernel_info["content"] == shell_kernel_info["content"]
    kernel_info = shell_kernel_info["content"]

    with open("Cargo.toml", "rb") as cargo_toml_file:
        cargo_toml = tomllib.load(cargo_toml_file)
        package = cargo_toml["package"]
        metadata = package["metadata"]["jupyter"]
        nu_engine = cargo_toml["workspace"]["dependencies"]["nu-engine"]
        assert kernel_info["protocol_version"] == metadata["protocol_version"]
        assert kernel_info["implementation"] == package["name"]
        assert kernel_info["implementation_version"] == package["version"]
        assert kernel_info["language_info"]["name"] == "nushell"
        assert kernel_info["language_info"]["version"] == nu_engine["version"]
        assert kernel_info["language_info"]["file_extension"] == ".nu"


def test_basic_rendering(kernel: BlockingKernelClient):
    contents = ok(kernel, "$nuju")
    assert len(contents) == 1
    data = contents[0]["data"]
    assert "application/json" in data
    assert "text/plain" in data
    assert "text/html" in data
    assert "text/markdown" in data


def test_http_get_https(kernel: BlockingKernelClient):
    contents = ok(kernel, 'http get "https://example.com"')
    assert len(contents) == 1
    html = contents[0]["data"]["text/plain"].strip()
    assert html.lower().startswith("<!doctype html>")
    assert "<title>Example Domain</title>" in html
    assert (
        "This domain is for use in documentation examples without needing permission."
        in html
    )
    assert html.endswith("</body></html>")


def test_nuju_content(kernel: BlockingKernelClient):
    contents = ok(kernel, "$nuju")
    assert len(contents) == 1
    data = contents[0]["data"]
    nuju_constant = json.loads(data["application/json"])
    with open("Cargo.toml", "rb") as cargo_toml_file:
        cargo_toml = tomllib.load(cargo_toml_file)
        assert nuju_constant["version"]["kernel"] == cargo_toml["package"]["version"]

        nu_version = cargo_toml["workspace"]["dependencies"]["nu-engine"]["version"]
        assert nuju_constant["version"]["nu"] == nu_version


def test_persistence(kernel: BlockingKernelClient):
    set_value = ok(kernel, "let foo = 'bar'")
    assert len(set_value) == 0
    
    get_value = ok(kernel, "$foo")
    assert len(get_value) == 1

    assert get_value[0]["data"]["text/plain"] == "bar"
