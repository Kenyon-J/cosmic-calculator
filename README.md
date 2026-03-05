# COSMIC Calculator

A feature-rich, multi-modal calculator designed natively for the [System76 COSMIC](https://github.com/pop-os/cosmic-epoch) Desktop Environment. Built with Rust and `cosmic-iced`, it seamlessly integrates with COSMIC's theming, rounded corners, and UI components while offering powerful tools for developers, scientists, and everyday users.

## ✨ Features

COSMIC Calculator includes five distinct modes to handle any workflow:

* **Standard Mode:** * Clean interface for quick, everyday arithmetic.
    * Built-in quick conversions for volume (pt/L, gal/L), distance (mi/km), and weight (lb/kg).
* **Scientific Mode:** * Evaluate complex string-based mathematical expressions.
    * Quick-insert menu for mathematical and physical constants (π, e, φ, √2, c, g, h, Na).
* **Programmer Mode:** * Seamlessly switch between HEX, DEC, OCT, and BIN bases.
    * Perform bitwise operations: `AND`, `OR`, `XOR`, `NOT`, and bit shifts (`<<`, `>>`).
    * Visual 64-bit breakdown, automatically grouped into readable nibbles.
* **RPN (Reverse Polish Notation) Mode:** * Stack-based calculation interface.
    * Visual 3-level stack history with `ENTER` and `DROP` controls.
* **Statistics Mode:** * Input a dataset to instantly calculate and view analytics.
    * Automatically derives `n`, `sum`, `mean`, `median`, `variance`, `min`, `max`, and `range`.

### Global Capabilities
* **Calculation History:** Stores up to 500 previous calculations. View your history and click any entry to copy it directly to your clipboard.
* **Native Theming:** Automatically adapts to your active COSMIC theme (accent colors, component backgrounds, and border radii).
* **Fully Keyboard Navigable:** Designed to be used without taking your hands off the keyboard.

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl` + `1` | Switch to Standard Mode |
| `Ctrl` + `2` | Switch to Scientific Mode |
| `Ctrl` + `3` | Switch to Programmer Mode |
| `Ctrl` + `4` | Switch to RPN Mode |
| `Ctrl` + `5` | Switch to Statistics Mode |
| `Enter` | Equals (`=`) / Enter (RPN) |
| `Escape` | Clear All (`C`) |
| `Delete` | Clear Entry (`CE`) |
| `Backspace`| Delete last character (`DEL`) |
| `a` - `f` | Hexadecimal entry (in Programmer Mode) |

## 🚀 Installation

### Building from Source

Ensure you have [Rust and Cargo](https://rustup.rs/) installed, as well as the necessary COSMIC/iced dependencies for your distribution.

```bash
git clone [https://github.com/Kenyon-J/cosmic-calculator.git](https://github.com/Kenyon-J/cosmic-calculator.git)
cd cosmic-calculator
cargo build --release
