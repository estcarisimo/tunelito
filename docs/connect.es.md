# Conectarse por SSH

Necesitás `cloudflared` en el `PATH`, y una clave pública que ya pegaron.

```bash
ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -C helper
```

Mandá solo la línea del `.pub`.

Tocan Copiar o Compartir. En **Detalles** está el bloque SSH, o escribilo vos:

```text
Host el-nombre.trycloudflare.com
  HostName el-nombre.trycloudflare.com
  User tunelito
  IdentityFile ~/.ssh/id_ed25519
  IdentitiesOnly yes
  StrictHostKeyChecking accept-new
  ProxyCommand cloudflared access ssh --hostname %h
```

```bash
ssh el-nombre.trycloudflare.com
```

El nombre cambia cada vez que lo encienden. `StrictHostKeyChecking accept-new` confía en la clave del host la primera vez que ve ese nombre.

## Claude Desktop

En macOS y Linux, Desktop lee `~/.ssh/config`. Agregá ese nombre y entrá como `tunelito`.

En Windows, el SSH que trae Desktop ignora ese archivo. Cambiarlo al OpenSSH del sistema **hay que verificarlo en tu versión**. Desde una terminal, `ssh` funciona. La nota del repo está en `docs/CLAUDE_DESKTOP.md`.

## Opcional

`tunelito-connect` puede imprimir una clave y mandar un hello sin verificar. No hace falta. Una sesión sin hello igual aparece, como **sin datos del cliente**.
