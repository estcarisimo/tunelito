# FAQ

## Do they need to be technical?

No. They open one file, accept one prompt, and paste the key you sent.

## Why not port forwarding?

The computer is behind NAT or CGNAT. There is no port on the router to forward.

## Do I need tunelito-connect?

No. `cloudflared`, a public key, and `ssh` are enough.

## It says "no client info". Is it broken?

No. That means they connected with plain SSH and sent no hello. The row still has the dot, the user, the fingerprint, and the times.

## Why is the address 127.0.0.1?

`cloudflared` on their computer connects to localhost. The real address is not visible there.

## Does it start at boot?

No. Closing the window turns it off. Opening the app does not turn it back on.

## Where does Claude Code install?

In the `tunelito` account, under `~/.claude/remote/`. That is one use of the session, not the product.
