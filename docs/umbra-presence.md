# Umbra player badge

Umbra draws its crescent-and-orbit mark above recognized players and beside their names in the
player list. The Settings → General → Network options let a player publish their badge visibility
and hide badge graphics locally. Lookups use the signed-in Bedrock account's roster identifiers;
server addresses and names are not sent. The badge indicates a recent presence-service match, not
proof of a particular running executable or a claim that the player is currently online.

The roster and privacy protocol currently comes from Cinnabar's API, which is the fallback when
`UMBRA_PRESENCE_API_BASE_URL` is unset. Its results are defined by that shared service and are
not scoped to an Umbra-only cohort. Set `UMBRA_PRESENCE_API_BASE_URL` to an Umbra-operated,
protocol-compatible service to use a separate backend. Such a service must authenticate the
multiplayer token, keep visibility opt-in, expire stale presence, rate-limit batched roster
lookups and return only the matches for the requested XUIDs/UUIDs. This repository does not host
that service.

The local display mark is Umbra-branded regardless of which compatible service supplies a match.
The rendering pipeline and roster lookup cache are already shared by the world name tags and the
player-list screen.
