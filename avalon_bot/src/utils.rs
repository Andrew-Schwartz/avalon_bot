use std::array::IntoIter;

/// List an iterator with a know size in a grammatically pleasing way, separated by commas and
/// with a word (likely "and" or "or") before the last element.
/// ```
/// let long_list = vec![1, 2, 3, 4];
/// let listed = long_list.into_iter().list_grammatically(i32::to_string, "and");
/// assert_eq!(listed, "1, 2, 3, and 4");
///
/// let short_list = vec![1, 2];
/// let listed = short_list.into_iter().list_grammatically(i32::to_string, "or");
/// assert_eq!(listed, "1 or 2");
///
/// let one_element = vec![1];
/// let listed = one_element.into_iter().list_grammatically(i32::to_string, "not_used");
/// assert_eq!(listed, "1");
/// ```
pub trait ListIterGrammatically: ExactSizeIterator + Sized {
    /// See the trait's documentation
    fn list_grammatically<F: FnMut(Self::Item) -> String>(self, to_string: F, word: &str) -> String {
        let last = self.len() - 1;
        self.map(to_string)
            .enumerate()
            .fold(String::new(), |mut acc, (i, new)| {
                if i != 0 {
                    if i == last {
                        if i == 1 {
                            acc.push(' ');
                            acc.push_str(word);
                            acc.push(' ');
                        } else {
                            acc.push_str(" , ");
                            acc.push_str(word);
                            acc.push(' ');
                        }
                    } else {
                        acc.push_str(", ");
                    }
                }
                acc.push_str(&new);
                acc
            })
    }
}

impl<I: ExactSizeIterator> ListIterGrammatically for I {}

pub trait StreamIter: IntoIterator + Sized {
    fn stream(self) -> futures::stream::Iter<Self::IntoIter> {
        futures::stream::iter(self)
    }
}

impl<I: IntoIterator> StreamIter for I {}

pub trait ArrayIter<T, const N: usize> {
    fn array_iter(self) -> std::array::IntoIter<T, N>;
}

impl<T, const N: usize> ArrayIter<T, N> for [T; N] {
    fn array_iter(self) -> IntoIter<T, N> {
        std::array::IntoIter::new(self)
    }
}