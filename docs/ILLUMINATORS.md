# Illuminators

## What an illuminator is

An **illuminator** is a WiFi transmitter that the sensing fleet does not
control but can hear. Every frame it sends crosses the space between it and
each receiving node. The receiver records that frame's channel state
information (CSI). When a person moves across or near that path, the CSI
changes.

This is passive bistatic sensing. The illuminator "lights" the space, the
nodes watch how that light is disturbed, and nobody has to install or power
the transmitter. One node hearing ten illuminators observes ten different
paths through the house, not one.

The term covers every transmitter apart from the fleet's own nodes. The access
point the nodes associate with is one; its links are drawn separately because
nearly every node hears it.

## Examples

Good illuminators are fixed and transmit regularly:

- the access point the nodes use, and other access points in the same house
- neighbours' routers and their extra BSSIDs (one radio often advertises
  several networks)
- fixed smart-home devices: thermostats, smoke alarms, smart speakers, hubs
- energy equipment: electricity meters, solar inverters, battery gateways,
  some of which run their own small WiFi network
- irrigation controllers, printers, TVs and streaming boxes that stay plugged in

Bad illuminators move or come and go:

- phones, watches, laptops and tablets
- anything that roams between rooms

A moving transmitter is worse than none. It teaches an association between a
CSI pattern and a location that is only true while it stands still.

## How the data is used

The pipeline follows the order the pieces were contributed:

1. **Transmitter identity on the wire.** CSI frame format v3 carries the
   transmitter's MAC and its sequence number, so every frame says who sent it
   (#1805 firmware, #1806 server parser).
2. **Per-link metrics.** The server keeps CSI statistics per
   (receiver, transmitter) pair, not per node (#1828). This "link table" is
   what everything else reads.
3. **Attribution by address.** A transmitter is identified by the address it
   reports, not by which one is loudest (#1829).
4. **Pairing.** Frames from the same transmission, heard by several nodes, are
   matched by (transmitter, sequence number) (#1808).
5. **Triage.** Each emitter has an explicit state (#1833):
   - `excluded`: it moves. Dropped at ingestion, so it leaves the link table
     and every consumer at once.
   - `pending`: heard but not judged. Used for position-free pattern matching,
     never as a geometric reference. Every new transmitter starts here.
   - `approved`: trusted to stay put, position estimated, with a non-zero
     uncertainty (you know which house a neighbour's router is in, not which
     room).
   - `surveyed`: trusted to stay put, position measured.

   Being loud is not evidence of standing still, so promotion is always a
   decision.
6. **Views.** The 3D view draws node-to-trusted-illuminator links and their
   co-reception counts (#1834).

## What one deployment measured

These come from a single 9-node house deployment. Treat them as warnings to
check, not general laws.

- **Many illuminators transmit rarely.** Smart-home devices were often heard
  at 0.01-0.1 frames per second, against tens of frames per second from the
  access point.
- **At those rates two receivers almost never capture the same frame**, so
  cross-receiver pairing (step 4) works for the access point and fails for
  most low-rate illuminators.
- **The population changes.** When access points reboot, the set of
  transmitters each node hears can change. Check that two time windows heard
  the same illuminators before comparing them.
- **Geometry does not predict response.** A model that expected a link to
  respond most when a person stands on the straight line between transmitter
  and receiver was falsified. Learning each link's own response pattern
  (fingerprinting) worked.
- **Coverage has holes.** Depending on where nodes and illuminators sit, some
  nodes can be blind to whole directions. Placement matters more than count.
