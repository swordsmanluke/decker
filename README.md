# decker
A terminal multiplexer intended for use in a heads up display.

Decker provides a configurable terminal session with an interactive pane and will also run as many scheduled read-only widgets as your heart desires. 

## Tl;Dr
This project is still under heavy development and mostly doesn't work yet.
Running passive 'widgets' works great. Running interactive shell sessions is unusably glitchy.
I'm working on it. Feel free to help out. You know, if you wanna. :D

## Why Yet Another Terminal Multiplexer?
Yep. There are a lot of them out there. Screen, Tmux, Tab-rs, Zellij, and so on. _This_ multiplexer is built around the idea of a main, interactive terminal session surrounded by text widgets.

(Besides, this is fun.)

## Widgets?
Yep. Widgets.

See, I wear this thing on my face: https://www.reddit.com/r/cyberDeck/comments/gao7hy/seattle_cyberpunk/

I want a stream of helpful, passive information that I can access at a glance, like the weather forecast, the time and so forth. These widgets then should be easy to configure so that I can change things up as I have new ideas about what to display. 

At the same time, I want a pane in which I can run interactive terminal applications like vim, or a custom To Do / Calendar day planner app. Or a barcode scanner to check online reviews and prices... and so on. 

## Can't you just run e.g. Tmux for that anyway?
Sure. I _could_. But like most multiplexers, Tmux has a status bar and other UI hints to help separate logical panes from one another. I don't want that. I only have about 72x19 characters on my HUD, so every line counts! Besides, I only have one pane that needs interaction - everything else would just be the equivalent of running `watch <some script>`.

## Why Rust?
Originally, I wrote v1 of what would become Decker in Kotlin! But it wasn't as performant as I'd like on my RasPi Zero W. So I figured I'd try out a compiled language and so long as I was at it... Let's try Rust!

## Project History
Decker represents the the fourth version (...and third renaming) of this project.
This is the first version that realizes my desire to have an interactive session. The previous 3 iterations supported read-only widgets, but did not support forwarding stdin and responding in any meaningful way to stdout.

