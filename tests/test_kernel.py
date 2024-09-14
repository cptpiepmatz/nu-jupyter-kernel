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


def ok(client: BlockingKernelClient, code: str):
    client.execute(code)
    iopub_reply = client.get_iopub_msg(timeout=10)
    shell_reply = client.get_shell_msg(timeout=10)
    assert shell_reply["content"]["status"] == "ok"
    return iopub_reply["content"]


def test_basic_rendering(kernel: BlockingKernelClient):
    content = ok(kernel, "$nuju")
    data = content["data"]
    assert "application/json" in data
    assert "text/plain" in data
    assert "text/html" in data
    assert "text/markdown" in data


def test_nuju_content(kernel: BlockingKernelClient):
    content = ok(kernel, "$nuju")
    data = content["data"]
    nuju_constant = json.loads(data["application/json"])
    with open("Cargo.toml", "rb") as cargo_toml_file:
        cargo_toml = tomllib.load(cargo_toml_file)
        assert nuju_constant["version"]["kernel"] == cargo_toml["package"]["version"]
        
        nu_version = cargo_toml["workspace"]["dependencies"]["nu-engine"]["version"]
        assert nuju_constant["version"]["nu"] == nu_version
