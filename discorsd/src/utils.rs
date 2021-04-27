// pub trait TryRemove<T> {
//     fn try_remove(&mut self, index: usize) -> Option<T>;
// }
//
// impl<T> TryRemove<T> for Vec<T> {
//     fn try_remove(&mut self, index: usize) -> Option<T> {
//         if index >= self.len() {
//             None
//         } else {
//             Some(self.remove(index))
//         }
//     }
// }

pub fn array_try_from_iter<T, Iterable, F, Error, const N: usize>(
    iterable: Iterable,
    mut not_enough_elements: F,
) -> Result<[T; N], Error>
    where
        Iterable: IntoIterator<Item=Result<T, Error>>,
    // really should be able to be `FnOnce` but try_array_init's signature can't show that
        F: FnMut(usize) -> Error,
{
    array_init::try_array_init({
        let mut iterator = iterable.into_iter();
        move |i| {
            match iterator.next() {
                Some(a) => a,
                None => Err(not_enough_elements(i)),
            }
        }
    })
}
