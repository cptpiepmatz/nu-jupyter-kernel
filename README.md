<h1 align="center">nu-jupyter-kernel</h1>
<p align="center">
  <b>
    A 
    <a href="https://jupyter.org">jupyter</a> 
    <a href=https://jupyter-client.readthedocs.io">raw kernel</a> 
    for <a href="https://www.nushell.sh">nu</a>.
  </b>
</p>

<br>

<p align="center">
  <a href="https://github.com/cptpiepmatz/nu-jupyter-kernel">
    <img alt="Version" src="https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fcptpiepmatz%2Fnu-jupyter-kernel%2Fmain%2FCargo.toml&query=package.version&prefix=v&style=for-the-badge&label=version"/>
  </a>
  <a href="https://github.com/nushell/nushell">
    <img alt="Nu Version" src="https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fcptpiepmatz%2Fnu-jupyter-kernel%2Fmain%2FCargo.toml&query=workspace.dependencies%5B'nu-engine'%5D.version&prefix=v&style=for-the-badge&label=nu%20version&color=%234E9A06"/>
  </a>
  <img alt="Jupyter Protocol Version" src="https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fcptpiepmatz%2Fnu-jupyter-kernel%2Fmain%2FCargo.toml&query=package.metadata.jupyter.protocol_version&prefix=v&style=for-the-badge&label=Jupyter%20Protocol%20Version&color=%23F37726"/>
  <a href="https://github.com/cptpiepmatz/nu-jupyter-kernel/blob/main/LICENSE">
    <img alt="License" src="https://img.shields.io/github/license/cptpiepmatz/nu-jupyter-kernel?style=for-the-badge"/>  
  </a>
</p>

## About
`nu-jupyter-kernel` is a [Jupyter](https://jupyter.org) kernel specifically for 
executing Nu pipelines. 
Unlike most Jupyter kernels that rely on Python, this raw kernel directly 
implements the Jupyter messaging protocol, allowing direct communication without 
Python intermediaries. 
It's designed to work seamlessly with Nu, the language utilized by 
[Nushell](https://github.com/nushell/nushell) — 
a modern shell that emphasizes structured data.

![screenshot](media/screenshot.png)

## Features
The `nu-jupyter-kernel` has several features making it a useful kernel for 
Jupyter notebooks:

- **Execution of Nu code:** 
  Directly run Nu pipeplines within your Jupyter notebook.

- **State sharing across cells:** 
  Unlike isolated script execution, the kernel maintains state across different 
  cells using the `nu-engine`.

- **Rich Data Rendering:** 
  Outputs are dynamically rendered in various data types wherever applicable.

- **Inline Value Printing:** 
  Easily print values at any point during cell execution.

- **Controlled External Commands:** 
  By default, external commands are disabled for reproducibility. 
  You can enable them as needed, and they will function as they do in Nushell.

- **Kernel Information:** 
  Access kernel-specific information via the `$nuju` constant.

- **Error representation:** 
  Shell errors are beautifully rendered.

- **Nushell Plugin Compatibility:** 
  Supports Nushell plugins within notebooks, allowing them to be loaded and 
  utilized as in a typical Nushell environment.

- **Plotting Integration:**
  The kernel directly integrates the `nu_plugin_plotters`, making plots easily 
  accessible.

## Examples
In the "examples" directory are some notebooks that show how the kernel works.
Opening the examples on Github also shows a preview of them.


## Design Goals
The design of the `nu-jupyter-kernel` focuses on the following goals:

- **Reproducibility:**
  Notebooks should be as reproducible as possible by default.

- **Clarity in dependencies:** 
  Make all dependencies clear and obvious to the user.

- **Script-like behavior:** 
  The kernel behaves largely like a regular Nu script to ensure familiarity.

- **Clear Feature Distinctions:** 
  Clearly indicating any deviations or limitations compared to standard Nu 
  script capabilities to avoid confusion during notebook executions.

## Installation
To build the kernel you need to have the rust toolchain installed, check the 
[installation guide on rust's official website](https://www.rust-lang.org/tools/install).

Using `cargo install nu-jupyter-kernel` you can install the latest release of 
the kernel.
If you want to install the latest version on the git repo, you can install the 
kernel via `cargo install nu-jupyter-kernel --git https://github.com/cptpiepmatz/nu-jupyter-kernel.git`

## Usage
### Registering the Kernel
After installation, you must register the kernel to make it available within 
Jupyter environments. 
This can be done through the command:

```sh
nu-jupyter-kernel register
```

You can specify the registration scope using `--user` for the current user 
(default) or `--system` for system-wide availability.

### Using the Kernel

- **Jupyter Notebook:** 
  Open Jupyter Notebook, create or open a notebook, and then select "Nushell" 
  from the kernel options in the top right corner.

- **Visual Studio Code:**
  Ensure you have the 
  [Jupyter extension by Microsoft](https://marketplace.visualstudio.com/items?itemName=ms-toolsai.jupyter) 
  installed. 
  Open a `.ipynb` file, click on "Select Kernel", choose "Jupyter Kernel", and 
  you should see "Nushell" listed. 

Both options may require a restart after registering the kernel.

### Note on Updates
Kernel binary updates do not require re-registration unless the binary's 
location changes. 
For developers, keep in mind that running `cargo run register` and 
`cargo run --release register` will result in different binary locations.

## Version Scheme
This crate follows the semantic versioning scheme as required by the
[Rust documentation](https://doc.rust-lang.org/cargo/reference/semver.html).
The version number is represented as `x.y.z+a.b.c`, where `x.y.z` is the version
of the crate and `a.b.c` is the version of the `nu-engine` that this crate is 
built with.
The `+` symbol is used to separate the two version numbers.


## Contributing
Contributions are welcome! 
If you're interested in contributing to the `nu-jupyter-kernel`, you can start 
by opening an issue or a pull request. 
If you'd like to discuss potential changes or get more involved, join the 
Nushell community on Discord. 
Invite links are available when you start Nushell or on their GitHub repository.

## Testing
For integration testing, this project uses a Python project set up with 
[`uv`](https://github.com/astral-sh/uv).
So make sure to have that installed.

Then do the following to run tests:
```sh
# register the current kernel
cargo run register

# sync the python dependencies
uv sync

# run the tests
uv run pytes
```

Tests are done via python as tools for executing Jupyter notebooks are currently 
not available in Rust.


This project uses [`uv`](https://github.com/astral-sh/uv) for integration 
testing. 
Since tools for executing Jupyter notebooks are not currently available in Rust, 
the tests are handled via Python.

To run the tests, follow these steps:
1. **Register the kernel**:
  ```nushell
  cargo run register
  ```
2. **Sync Python dependencies:**
  ```nushell
  uv sync
  ```
3. **Run the tests:**
  ```nushell
  uv run pytest
  ```

Make sure `uv` is installed before running the commands.
