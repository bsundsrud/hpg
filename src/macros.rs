#[macro_export]
macro_rules! output {
    ($($arg:tt)+) => ({
        use $crate::tracker::Tracker;
        $crate::tracker::tracker().println(format_args!($($arg)+));
    });
}

#[macro_export]
macro_rules! indent_output {
    ($level:expr, $($arg:tt)+) => ({
        use $crate::tracker::Tracker;
        $crate::tracker::tracker().indent_println($level, format_args!($($arg)+));
    });
}

#[macro_export]
macro_rules! debug_output {
    ($($arg:tt)+) => ({
        use $crate::tracker::Tracker;
        $crate::tracker::tracker().debug_println(format_args!($($arg)+));
    });
}
