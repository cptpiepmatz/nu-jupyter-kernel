import json

import pytest
import tomllib
from jupyter_client import BlockingKernelClient, KernelManager


@pytest.fixture
def kernel():
    km = KernelManager(kernel_name="nu")
    km.start_kernel()
    yield km.client()
    km.shutdown_kernel()


def ok(client: BlockingKernelClient, code: str) -> list[dict]:
    # wait until client is ready, then send some code
    client.wait_for_ready(timeout=120)
    client.execute(code)

    # kernel should instantly reply with busy message
    busy_status = client.get_iopub_msg(timeout=120)
    assert busy_status["content"]["execution_state"] == "busy"

    # check on the iopub channel until we receive an idle message
    contents = []
    while True:
        iopub_reply = client.get_iopub_msg(timeout=120)
        if iopub_reply["content"].get("execution_state") == "idle": break
        contents.append(iopub_reply["content"])

    # we should get a ok on the shell channel
    shell_reply = client.get_shell_msg(timeout=120)
    assert shell_reply["content"]["status"] == "ok"

    return contents


def test_basic_rendering(kernel: BlockingKernelClient):
    contents = ok(kernel, "$nuju")
    assert len(contents) == 1
    data = contents[0]["data"]
    assert "application/json" in data
    assert "text/plain" in data
    assert "text/html" in data
    assert "text/markdown" in data


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
