#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}

#[macro_export]
macro_rules! log_warning {
    ($($arg:tt)*) => {{
        print!("\x1b[33m");
        println!($($arg)*);
        print!("\x1b[39m");
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        print!("\x1b[31m");
        println!($($arg)*);
        print!("\x1b[39m");
    }};
}

#[macro_export]
macro_rules! log_success {
    ($($arg:tt)*) => {{
        print!("\x1b[32m");
        println!($($arg)*);
        print!("\x1b[39m");
    }};
}
