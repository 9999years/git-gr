# git-gr

[![Crates.io](https://img.shields.io/crates/v/git-gr)](https://crates.io/crates/git-gr)

A Git [Gerrit][gerrit] command-line client.

[gerrit]: https://www.gerritcodereview.com/

[![Terminal recording demonstrating various git-gr features](https://asciinema.org/a/682490.svg)](https://asciinema.org/a/682490)

## Commands

- `push`: Push your current branch to Gerrit
- `checkout CL`: Checkout a CL by number
- `fetch CL`: Fetch a CL by number
- `view [CL]`: View a CL, by default the current CL, in your web browser 
- `query [--mine|--needs-review] [QUERY]`: Search for CLs

### Stacks

One of Gerrit's best features is its native support for stacks of CLs. However,
it's not always easy to keep stacks up to date. `git-gr` provides a number of
tools to help:

- `restack`: Restack CLs, updating CLs against the base branch and rebasing
  subsequent CLs on previous ones.
  - `restack push`: Push a stack of CLs to Gerrit after restacking
  - `restack this`: Restack a single CL on its immediate parent
  - `restack continue` Continue an in-progress restack after fixing conflicts
  - `restack abort` Abort an in-progress restack instead of fixing conflicts
- `up`: Checkout this CL's parent
- `down`: Checkout this CL's child
- `top`: Checkout the top-most CL in the current stack (this CL will be
  targeting the base branch and can be merged next)

### API Access

`git-gr` also offers several lower-level utility commands:

- `cli`: Run a `gerrit` command on the remote server
- `api`: Make a request to the Gerrit REST API
