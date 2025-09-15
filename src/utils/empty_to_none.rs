
/// Convert Option::Some() to None if the contained value is empty
pub trait EmptyToNone<T> {
    fn empty_to_none(self) -> Option<T>;
}

macro_rules! empty_to_none_impl {
    ($ty:ty) => {
        fn empty_to_none(self) -> Option<$ty> {
            match self {
                None => None,
                Some(s) if s.is_empty() => None,
                Some(s) => Some(s),
            }
        }
    };
}

impl<'a> EmptyToNone<&'a str> for Option<&'a str> {
    empty_to_none_impl!(&'a str);
}

impl<'a> EmptyToNone<&'a String> for Option<&'a String> {
    empty_to_none_impl!(&'a String);
}

impl EmptyToNone<String> for Option<String> {
    empty_to_none_impl!(String);
}

impl<X> EmptyToNone<Vec<X>> for Option<Vec<X>> {
    empty_to_none_impl!(Vec<X>);
}

impl<'a, X> EmptyToNone<&'a Vec<X>> for Option<&'a Vec<X>> {
    empty_to_none_impl!(&'a Vec<X>);
}
