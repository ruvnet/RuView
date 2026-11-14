//! Macros used internally.
//!
//! These may technically be exported, but that's only to make them available to internal
//! project dependencies. The `#[doc(hidden)]` mark indicates that these are not stable or supported
//! APIs, and should not be relied upon by external dependees.

/// The single macro export of the [`cfg-if`](https://docs.rs/cfg-if) crate.
///
/// It is packaged here to avoid pulling in another dependency. The stdlib does the same[^1].
///
/// [^1]: https://github.com/rust-lang/rust/blob/a2db9280539229a3b8a084a09886670a57bc7e9c/library/compiler-builtins/libm/src/math/support/macros.rs#L1
#[doc(hidden)]
#[macro_export]
macro_rules! cfg_if {
    // match if/else chains with a final `else`
    (
        $(
            if #[cfg( $i_meta:meta )] { $( $i_tokens:tt )* }
        ) else+
        else { $( $e_tokens:tt )* }
    ) => {
        $crate::cfg_if! {
            @__items () ;
            $(
                (( $i_meta ) ( $( $i_tokens )* )) ,
            )+
            (() ( $( $e_tokens )* )) ,
        };
    };

    // match if/else chains lacking a final `else`
    (
        if #[cfg( $i_meta:meta )] { $( $i_tokens:tt )* }
        $(
            else if #[cfg( $e_meta:meta )] { $( $e_tokens:tt )* }
        )*
    ) => {
        $crate::cfg_if! {
            @__items () ;
            (( $i_meta ) ( $( $i_tokens )* )) ,
            $(
                (( $e_meta ) ( $( $e_tokens )* )) ,
            )*
        };
    };

    // Internal and recursive macro to emit all the items
    //
    // Collects all the previous cfgs in a list at the beginning, so they can be
    // negated. After the semicolon are all the remaining items.
    (@__items ( $( $_:meta , )* ) ; ) => {};
    (
        @__items ( $( $no:meta , )* ) ;
        (( $( $yes:meta )? ) ( $( $tokens:tt )* )) ,
        $( $rest:tt , )*
    ) => {
        // Emit all items within one block, applying an appropriate #[cfg]. The
        // #[cfg] will require all `$yes` matchers specified and must also negate
        // all previous matchers.
        #[cfg(all(
            $( $yes , )?
            not(any( $( $no ),* ))
        ))]
        $crate::cfg_if! { @__identity $( $tokens )* }

        // Recurse to emit all other items in `$rest`, and when we do so add all
        // our `$yes` matchers to the list of `$no` matchers as future emissions
        // will have to negate everything we just matched as well.
        $crate::cfg_if! {
            @__items ( $( $no , )* $( $yes , )? ) ;
            $( $rest , )*
        };
    };

    // Internal macro to make __apply work out right for different match types,
    // because of how macros match/expand stuff.
    (@__identity $( $tokens:tt )* ) => {
        $( $tokens )*
    };
}

/// Similar to [`cfg_if`](cfg_if), but accepts a list of expressions, and generates an internal
/// closure to return each value.
///
/// The main reason this is necessary is because attaching `#[cfg(...)]` annotations to certain
/// types of statements requires a nightly feature, or `cfg_if` would be enough on its own. This
/// macro's restricted interface allows it to generate a closure as a circumlocution that is legal
/// on stable rust.
///
/// Note that any `return` operation within the expressions provided to this macro will apply to the
/// generated closure, not the enclosing scope--it cannot be used to interfere with external
/// control flow.
///
/// The generated closure is non-[`const`](const@keyword), so cannot be used inside `const` methods.
#[doc(hidden)]
#[macro_export]
macro_rules! cfg_if_expr {
    // Match =>, chains, maybe with a final _ => catchall clause.
    (
        $( $ret_ty:ty : )?
        $(
            #[cfg( $i_meta:meta )] => $i_val:expr
        ),+ ,
            _ => $rem_val:expr $(,)?
    ) => {
        (|| $( -> $ret_ty )? {
            $crate::cfg_if_expr! {
                @__items ();
                $(
                    (( $i_meta ) (
                        #[allow(unreachable_code)]
                        return $i_val ;
                    )) ,
                )+
                    (() (
                        #[allow(unreachable_code)]
                        return $rem_val ;
                    )) ,
            }
        })()
    };
    // Match =>, chains *without* any _ => clause.
    (
        $( $ret_ty:ty : )?
        $(
            #[cfg( $i_meta:meta )] => $i_val:expr
        ),+ $(,)?
    ) => {
        (|| $( -> $ret_ty )? {
            $crate::cfg_if_expr! {
                @__items ();
                $(
                    (( $i_meta ) (
                        #[allow(unreachable_code)]
                        return $i_val ;
                    )) ,
                )+
            }
        })()
    };

    (@__items ( $( $_:meta , )* ) ; ) => {};
    (
        @__items ( $( $no:meta , )* ) ;
        (( $( $yes:meta )? ) ( $( $tokens:tt )* )) ,
        $( $rest:tt , )*
    ) => {
        #[cfg(all(
            $( $yes , )?
            not(any( $( $no ),* ))
        ))]
        $crate::cfg_if_expr! { @__identity $( $tokens )* }

        $crate::cfg_if_expr! {
            @__items ( $( $no , )* $( $yes , )? ) ;
            $( $rest , )*
        };
    };
    (@__identity $( $tokens:tt )* ) => {
        $( $tokens )*
    };
}
