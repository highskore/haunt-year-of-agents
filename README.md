<div align="center">

<img src="docs/icon.png" width="96" alt="">

# Year of agents

**A calendar heatmap of your finished agent turns in Haunt, one square per day.**

[![Haunt plugin](https://img.shields.io/badge/Haunt-plugin-D97757)](https://github.com/highskore/haunt-hackathon)
![Plugin API 0.5](https://img.shields.io/badge/plugin%20API-0.5-111)
![WASM component](https://img.shields.io/badge/WASM-component-654ff0?logo=webassembly&logoColor=white)
![MIT licence](https://img.shields.io/badge/licence-MIT-6b7280)

</div>

Run **Year of agents** from the ⌘K palette and a pane titled **Last 26 weeks** opens beside the terminal, Sunday on top, today outlined in orange. Squares shade in fixed steps: 1-2, 3-5, 6-10 and 11 or more finished turns. Under the grid: total turns in view, your current streak (days in a row with a finished turn, counting from today or yesterday) and your busiest day. The pane updates as turns finish while it is open.

A turn counts when the agent's `stop` hook fires. Days follow your local time zone. Counts start when you install the plugin; nothing is back-filled. Only the count per day is stored, for about a year.

## Settings

```toml
[plugins."dev.haunt.year-of-agents"]
weeks = 26          # 1 to 53
stats = true        # turns, streak and busiest day under the grid
unverified = false  # also count stops without the tab's nonce
```

## Capabilities

| capability | why |
|---|---|
| `events = ["agent.stop"]` | to count finished turns; no event text is kept |
| `panel` | the pane |
| `timer` | the local time zone, so a day starts at your midnight |

Events without the tab's nonce are ignored unless `unverified` is on, since anything printed in the terminal can forge them.

## Build and test

```sh
cargo build --release --target wasm32-wasip2   # or ./build.sh
cargo test                                     # unit tests
haunt plugin install .
```

`e2e/year_of_agents.rs` runs the built plugin against a real Haunt core: copy it into a Haunt checkout's `cli/tests/` and run `HAUNT_PLUGINS_DIR=<this repo's parent> cargo test -p haunt-cli --test year_of_agents`.

Needs Haunt with plugin API 0.5 (`api = "0.5"`).

## Licence

MIT, see `LICENSE`.
