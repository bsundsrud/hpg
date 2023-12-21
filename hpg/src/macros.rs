#[macro_export]
macro_rules! output {
    ($($arg:tt)+) => ({
        use $crate::tracker::Tracker;
        $crate::tracker::TRACKER.get().unwrap().println(format_args!($($arg)+));
    });
}

#[macro_export]
macro_rules! indent_output {
    ($level:expr, $($arg:tt)+) => ({
        use $crate::tracker::Tracker;
        $crate::tracker::TRACKER.get().unwrap().indent_println($level, format_args!($($arg)+));
    });
}

#[macro_export]
macro_rules! debug_output {
    ($($arg:tt)+) => ({
        use $crate::tracker::Tracker;
        $crate::tracker::TRACKER.get().unwrap().debug_println(format_args!($($arg)+));
    });
}
