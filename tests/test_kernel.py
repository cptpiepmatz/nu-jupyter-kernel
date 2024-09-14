import json

import pytest
import tomllib
from jupyter_client import BlockingKernelClient, KernelManager


TIMEOUT = 10


@pytest.fixture
def kernel():
    km = KernelManager(kernel_name="nu")
    km.start_kernel()
    yield km.client()
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


def test_persistantance(kernel: BlockingKernelClient):
    set_value = ok(kernel, "let foo = 'bar'")
    assert len(set_value) == 0
    
    get_value = ok(kernel, "$foo")
    assert len(get_value) == 1

    assert get_value[0]["data"]["text/plain"] == "bar"
