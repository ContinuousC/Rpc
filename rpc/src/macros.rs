/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

/// Wait for a (cancellable) future or a termination signal.
/// Returns:
///    - Some(output), if the future returns first
///    - None, if the term_receiver goes to true first
///
/// abortable!(
///    term_receiver: mpsc::Receiver<bool>,
///    future: F
/// ) -> Option<Fut::Output>
/// where
///    F: Fn() -> Fut
///    Fut: Future
#[macro_export]
macro_rules! abortable {
    ($term_receiver:ident, $future:expr) => {
        loop {
            if *$term_receiver.borrow() {
                break None;
            }
            tokio::select! {
                r = $future => break Some(r),
                c = $term_receiver.changed() => match c {
                    Ok(()) => continue,
                    Err(_) => break None
                }
            }
        }
    };
}

#[macro_export]
macro_rules! pin_abortable {
    ($term_receiver:ident, $future:expr) => {{
        let future = $future;
        tokio::pin!(future);
        $crate::abortable!($term_receiver, &mut future)
    }};
}

#[macro_export]
macro_rules! abortable_sleep {
    ($term_receiver:ident, $duration:expr) => {
        $crate::pin_abortable!($term_receiver, tokio::time::sleep($duration))
    };
}
