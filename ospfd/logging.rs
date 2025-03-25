#[macro_export]
macro_rules! log {
    () => {{
        println!("\r\x1b[2K")
    }};
    ($($arg:tt)*) => {{
        println!("\r\x1b[2K{}", format!($($arg)*))
    }};
}

#[macro_export]
macro_rules! log_warning {
    ($($arg:tt)*) => {{
        print!("\r\x1b[2K\x1b[33m");
        print!($($arg)*);
        print!("\x1b[39m");
        println!()
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        print!("\r\x1b[2K\x1b[31m");
        print!($($arg)*);
        print!("\x1b[39m");
        println!()
    }};
}

#[macro_export]
macro_rules! log_success {
    ($($arg:tt)*) => {{
        print!("\r\x1b[2K\x1b[32m");
        print!($($arg)*);
        print!("\x1b[39m");
        println!()
    }};
}
