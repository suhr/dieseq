# Dieseq

![2018-02-07-014427_1149x715_scrot](https://user-images.githubusercontent.com/65870/35888429-7e97491c-0b8f-11e8-9d09-72857b168fd0.png)

Dieseq is a simple microtonal sequencer.

## Instalation

1. Install [Rust](https://www.rust-lang.org/)
2. `cargo install --git https://github.com/suhr/dieseq.git`

## Usage

To use dieseq, first you need to install [med](https://github.com/suhr/med).

Dieseq/med doesn't play any sound. Instead it send midi commands that can be executed by a synth.

Controls:

- Left mouse button allows to draw or select notes. Right mouse button drags the view.
- Mouse scroll changes the horizontal scale. With <kbd>Ctrl</kbd> it changes the vertial scale.
- <kbd>Space</kbd>: start/stop playing
- <kbd>1</kbd>: choose the arrow tool
- <kbd>2</kbd>: choose the pencil tool
- <kbd>d</kbd>: delete the selected notes
- <kbd>s</kbd>: save file
