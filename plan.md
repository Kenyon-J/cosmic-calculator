1. **Define `StyleContext` struct**: Create a `struct StyleContext<'a>` holding `rad`, `rad_m`, the colors (like `disp_bg`, `disp_fg`, etc.), and the common button helper closures (`d`, `o`, `a`, `dim`, `eq`, etc.) that are currently destructured from a large tuple in `fn view(&self)`.
2. **Update `view` method**: Refactor `fn view(&self)` to populate the `StyleContext` struct instead of an ad-hoc tuple and passing a ton of parameters to child view methods.
3. **Refactor view helper methods**: Change `view_standard_sci`, `view_programmer`, `view_rpn`, `view_statistics`, and `view_history` to take `&StyleContext` instead of a huge list of parameters.
4. **Fix calls to helper methods**: Update `fn view(&self)` to pass the single `&StyleContext` parameter to the helper methods.
5. **Run pre-commit steps**: `cargo check`, `cargo fmt`, `cargo clippy`.
6. **Submit**: Create PR.
