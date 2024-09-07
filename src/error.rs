pub enum KernelError {
    MissingFormatDecls {
        missing: Vec<&'static str>
    }
}
