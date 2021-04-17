/// List an iterator with a know size in a grammatically pleasing way, separated by commas and
/// with a word (likely "and" or "or") before the last element.
/// ```rust
/// let long_list = vec![1, 2, 3, 4];
/// println!("{}", long_list.into_iter().list_grammatically(i32::to_string, "and"));
/// // 1, 2, 3, and 4
///
/// let short_list = vec![1, 2];
/// println!("{}", one_element.into_iter().list_grammatically(i32::to_string, "or"));
/// // 1 or 2
/// ```
pub trait IterExt: ExactSizeIterator + Sized {
    /// List an iterator with a know size in a grammatically pleasing way, separated by commas and
    /// with a word (likely "and" or "or") before the last element.
    /// ```rust
    /// let long_list = vec![1, 2, 3, 4];
    /// println!("{}", long_list.into_iter().list_grammatically(i32::to_string, "and"));
    /// // 1, 2, 3, and 4
    ///
    /// let short_list = vec![1, 2];
    /// println!("{}", one_element.into_iter().list_grammatically(i32::to_string, "or"));
    /// // 1 or 2
    /// ```
    fn list_grammatically<F: FnMut(Self::Item) -> String>(self, to_string: F, word: &str) -> String {
        let last = self.len() - 1;
        self.map(to_string)
            .enumerate()
            .fold(String::new(), |mut acc, (i, new)| {
                if i != 0 {
                    if i == last {
                        if i == 1 {
                            acc.push_str(" ");
                            acc.push_str(word);
                            acc.push_str(" ");
                        } else {
                            acc.push_str(" , ");
                            acc.push_str(word);
                            acc.push_str(" ");
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

impl<I: ExactSizeIterator> IterExt for I {}

pub trait StreamIter: IntoIterator + Sized {
    fn stream(self) -> tokio::stream::Iter<Self::IntoIter> {
        tokio::stream::iter(self)
    }
}

impl<I: IntoIterator> StreamIter for I {}