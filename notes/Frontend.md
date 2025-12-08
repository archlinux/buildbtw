# buildbtw frontend stack

This document outlines our chosen technology and development style for making buildbtw's web UI.

## server-side HTML rendering

- Lower maintenance than SPAs
- Better accessibility by default
- Usually performs better (which can be counterintuitive)

## Datastar for sprinkles of interactivity

These kinds of tools (htmx, unpoly, hotwired, ...) are easily interchangeable, so it's not very important what we pick

## Tera template engine

Doesn't need a full recompilation which is essential when doing iterative work.

## Styling

- We don't want to invest much time into styling
- UI Constraints: should not look outdated, be accessible and intuitive
- Use CSS layers to ensure our own customizations can be easily applied on top of whatever component library we use
- Build a small PoC page using Bulma to see if it has what we need
- If we need to use custom styles beyond our chosen component library, use [encre](https://encrecss.uk.to/): The utility-class approach results in better locality of behavior, making it easier to understand and refactor styles
