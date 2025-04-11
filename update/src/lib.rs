pub mod macros {
    pub use update_derive::Update;
}

pub trait Update {
    fn update(&mut self, other: Self);
    fn remove<T: AsRef<str>>(&mut self, properties_name: &[T]);
}
