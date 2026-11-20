import { describe, expect, mock, test, tier } from 'claude-code/testing'

tier('user')

const CAPTURE = {
  ok: true,
  packets: 120,
  decodedPackets: 120,
  heartbeatOnlySenders: [{ address: '192.168.1.67', heartbeats: 30 }],
  nodes: [
    { source: 'esp32', nodeId: 42, csiRateHz: 4.7, csiLossFraction: 0, rssiMean: -63, csi: { shape: '1x64' } },
    { source: 'realtek', nodeId: 3, csiRateHz: 348, csiLossFraction: 0.08, rssiMean: -39, csi: { shape: '1x52' } },
  ],
}

describe('register', () => {
  test('/ruview opens the pane, refreshes from the harness CLI, and stops on close', async ($, on) => {
    const runs: (readonly string[])[] = []
    const opened: string[] = []
    const closed: string[] = []
    const statuses: (string | undefined)[] = []
    const clock = mock.clock(on)
    on('session.start', ($, e) => ({ cwd: e.cwd }))
    on('command.register', ($, e) => ({ value: { command: e.name } }))
    on('process.run', ($, e) => {
      runs.push(e.argv)
      return { value: { exitCode: 0, stdout: JSON.stringify(CAPTURE), stderr: '' } }
    })
    const focusAsked: (true | undefined)[] = []
    on('ui.open', ($, e) => {
      opened.push(e.id)
      focusAsked.push(e.focus)
      return { value: undefined }
    })
    on('ui.close', ($, e) => {
      closed.push(e.id)
      return { value: undefined }
    })
    on('ui.status', ($, e) => {
      statuses.push(e.text)
      return { value: undefined }
    })
    on('ui.invalidate', () => ({ value: undefined }))

    const settle = async () => {
      for (let i = 0; i < 5; i++) await clock.settle()
    }

    await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
    const { text } = await $.command.run({ command: 'ruview', args: '', origin: { kind: 'composer' } })
    await settle()

    expect(text).toContain('RuView pane open')
    expect(opened).toEqual(['ruview-live'])
    expect(focusAsked).toEqual([true]) // hotkeys reach a Pane only while it holds the keyboard
    expect(runs.length).toBeGreaterThanOrEqual(1)
    const argv = runs[0] ?? []
    expect(argv[0]).toBe('node')
    expect(String(argv[1])).toContain('bin/cli.js')
    expect(argv.slice(2)).toEqual(['esp32', '--seconds', '3', '--udp-port', '5005', '--json'])
    expect(statuses.at(-1)).toBe('RuView · 2 nodes · 1 alert')

    const beforeTicks = runs.length
    for (let i = 0; i < 3; i++) {
      await clock.advance(15_000)
      await settle()
    }
    expect(runs.length).toBeGreaterThan(beforeTicks)

    await $.command.run({ command: 'ruview', args: '', origin: { kind: 'composer' } })
    await settle()
    expect(closed).toEqual(['ruview-live'])
    const afterClose = runs.length
    for (let i = 0; i < 4; i++) {
      await clock.advance(15_000)
      await settle()
    }
    expect(runs.length).toBe(afterClose)
  })

  test('a pane drawn after a reload resumes polling instead of waiting forever', async ($, on) => {
    // Seen live: a hot reload re-runs register with fresh variables while the
    // engine keeps the pane open, so the drawing sat at "waiting for the first
    // capture…" with no timer running.
    const runs: (readonly string[])[] = []
    const clock = mock.clock(on)
    on('session.start', ($, e) => ({ cwd: e.cwd }))
    on('command.register', ($, e) => ({ value: { command: e.name } }))
    on('process.run', ($, e) => {
      runs.push(e.argv)
      return { value: { exitCode: 0, stdout: JSON.stringify(CAPTURE), stderr: '' } }
    })
    on('ui.status', () => ({ value: undefined }))
    on('ui.invalidate', () => ({ value: undefined }))
    on('ui.blit', () => ({ value: {} }))
    const PANE = {
      title: 'RuView',
      isFocused: false,
      bodyColumns: 100,
      placement: 'dock' as const,
      scroll: { offset: 0, bodyRows: 30 },
      view: {},
    }

    await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
    // No /ruview in this module instance: the engine draws the pane it kept open.
    const first = await $.ui.mount({ plugin: 'ruview-live', surface: 'terminal', component: 'Pane', requestId: 'ruview-live', props: PANE })
    for (let i = 0; i < 5; i++) await clock.settle()
    expect(runs.length).toBeGreaterThanOrEqual(1)
    await first.unmount()

    const again = await $.ui.mount({ plugin: 'ruview-live', surface: 'terminal', component: 'Pane', requestId: 'ruview-live', props: PANE })
    expect(await again.find({ type: 'Text', text: /NODES/ })).toBeDefined()
    expect(await again.find({ type: 'Text', text: /waiting for the first capture/ })).toBeUndefined()
    expect(await again.find({ type: 'Text', text: /ctrl\+x tab/ })).toBeDefined() // unfocused: says how to focus
    const polled = runs.length
    // Step time as a live session does (the animation timer runs every 80 ms).
    for (let s = 0; s < 16; s++) {
      await clock.advance(1_000)
      for (let i = 0; i < 5; i++) await clock.settle()
    }
    expect(runs.length).toBeGreaterThan(polled)
    await again.unmount()
  })

  test('the waterfall tab asks for spectrum frames, draws a Raster, and animates it with blits', async ($, on) => {
    const runs: (readonly string[])[] = []
    const blits: string[] = []
    const clock = mock.clock(on)
    const frames = Array.from({ length: 40 }, (_, k) => [k, 2 * k, 3 * k, 4 * k])
    on('session.start', ($, e) => ({ cwd: e.cwd }))
    on('command.register', ($, e) => ({ value: { command: e.name } }))
    on('process.run', ($, e) => {
      runs.push(e.argv)
      const spectrum = e.argv.includes('--spectrum')
        ? [{ source: 'esp32', nodeId: 42, subcarriers: 64, bins: 4, rateHz: 20, synthetic: false, frames }]
        : undefined
      return { value: { exitCode: 0, stdout: JSON.stringify({ ...CAPTURE, spectrum }), stderr: '' } }
    })
    on('ui.status', () => ({ value: undefined }))
    // No ui.invalidate stub here: the mounted drawing follows the mod's invalidations.
    on('ui.blit', ($, e) => {
      blits.push(e.key)
      return { value: {} }
    })
    const PANE = { title: 'RuView', isFocused: true, bodyColumns: 120, placement: 'dock' as const, scroll: { offset: 0, bodyRows: 34 }, view: {} }
    const settle = async () => {
      for (let i = 0; i < 5; i++) await clock.settle()
    }

    await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
    const pane = await $.ui.mount({ plugin: 'ruview-live', surface: 'terminal', component: 'Pane', requestId: 'ruview-live', props: PANE })
    await settle()
    expect(await pane.find({ key: 'tab-waterfall' })).toBeDefined()
    await pane.press({ key: 'tab-waterfall' })
    await settle()
    expect(runs.at(-1)).toContain('--spectrum')
    expect(runs.at(-1)).toContain('--spectrum-bins')

    expect(await pane.find({ type: 'Text', text: /MEASURED/ })).toBeDefined()
    expect(await pane.find({ type: 'Raster', key: 'waterfall' })).toBeDefined()
    for (let i = 0; i < 4; i++) {
      await clock.advance(100)
      await settle()
    }
    expect(blits).toContain('waterfall')
    expect(blits).toContain('shimmer')
    await pane.unmount()
  })

  test('/ruview refresh with no nodes reports the honest failure in the status', async ($, on) => {
    const statuses: (string | undefined)[] = []
    mock.clock(on)
    on('session.start', ($, e) => ({ cwd: e.cwd }))
    on('command.register', ($, e) => ({ value: { command: e.name } }))
    on('process.run', () => ({ value: { exitCode: 1, stdout: JSON.stringify({ ok: false, reason: 'no_packets', packets: 0 }), stderr: '' } }))
    on('ui.status', ($, e) => {
      statuses.push(e.text)
      return { value: undefined }
    })
    on('ui.invalidate', () => ({ value: undefined }))

    await $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
    const { text } = await $.command.run({ command: 'ruview', args: 'refresh', origin: { kind: 'composer' } })

    expect(text).toBe('RuView · 0 nodes')
    expect(statuses.at(-1)).toBe('RuView · 0 nodes')
  })
})
