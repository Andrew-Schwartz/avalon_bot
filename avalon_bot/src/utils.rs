pub trait IterExt: ExactSizeIterator + Sized {
    fn list_grammatically<F: FnMut(Self::Item) -> String>(self, to_string: F) -> String {
        let last = self.len() - 1;
        self.map(to_string)
            .enumerate()
            .fold(String::new(), |mut acc, (i, new)| {
                if i != 0 {
                    acc.push_str(if i == last {
                        if i == 1 {
                            " and "
                        } else {
                            " , and "
                        }
                    } else {
                        ", "
                    });
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