#[macro_export]
macro_rules! output {
    ($($arg:tt)+) => ({
        $crate::tracker::TRACKER.println(format_args!($($arg)+));
    });
}

#[macro_export]
macro_rules! indent_output {
    ($level:expr, $($arg:tt)+) => ({
        $crate::tracker::TRACKER.indent_println($level, format_args!($($arg)+));
    });
}
