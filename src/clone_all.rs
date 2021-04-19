// some convenience macros

#[macro_export]
macro_rules! clone_all {
    ($($i:ident),+) => {
        $(let $i = $i.clone();)+
    }
}

#[macro_export]
macro_rules! clone_all_mut {
    ($($i:ident),+) => {
        $(let mut $i = $i.clone();)+
    }
}
