import './style.css'
import EXAMPLE_NOTE from '@/masm/notes/EXAMPLE_NOTE.masm?raw'
import acc0 from '@/masm/accounts/acc0.masm?raw'
import lib1 from '@/masm/lib/lib1.masm?raw'
import lib0 from '@/masm/lib/lib0.masm?raw'
import lib2 from '@/masm/lib/lib2.masm?raw'

import { Linking, MidenClient } from '@miden-sdk/miden-sdk'

const appEl = document.querySelector<HTMLDivElement>('#app')!

function escapeHtml(s: string): string {
  return s
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const copy = new Uint8Array(bytes)
  const digest = await crypto.subtle.digest('SHA-256', copy)
  return [...new Uint8Array(digest)]
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

function extractLikelyProcedureHexLines(mastText: string): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  const reBare = /^[0-9a-f]{64}$/i
  const re0x = /^0x[0-9a-f]{64}$/i
  for (const line of mastText.split('\n')) {
    const t = line.trim()
    let hex64: string | undefined
    if (re0x.test(t)) {
      hex64 = t.slice(2).toLowerCase()
    } else if (reBare.test(t)) {
      hex64 = t.toLowerCase()
    }
    if (hex64 && !seen.has(hex64)) {
      seen.add(hex64)
      out.push(`0x${hex64}`)
    }
  }
  return out
}

async function run(): Promise<void> {
  appEl.innerHTML = '<p class="status">Initializing Miden compiler…</p>'

  const client = await MidenClient.createMock()

  try {
    appEl.innerHTML =
      '<p class="status">Compiling <code>EXAMPLE_NOTE.masm</code> with static libraries…</p>'

    const noteScript = await client.compile.noteScript({
      code: EXAMPLE_NOTE,
      libraries: [
                {
          namespace: 'sandbox::lib0',
          code: lib0,
          linking: Linking.Static,
        },
        {
          namespace: 'sandbox::lib1',
          code: lib1,
          linking: Linking.Static,
        },
        {
          namespace: 'sandbox::lib2',
          code: lib2,
          linking: Linking.Static,
        },
        {
          namespace: 'sandbox::acc0',
          code: acc0,
          linking: Linking.Static,
        },
      ],
    })

    const rootWord = noteScript.root()
    const noteRootHex = rootWord.toHex()
    rootWord.free()

    const serialized = noteScript.serialize()
    const serializedForDownload = Uint8Array.from(serialized)
    const bodySha256 = await sha256Hex(serializedForDownload)

    const mastText = noteScript.toString()
    const guessedDigests = extractLikelyProcedureHexLines(mastText)

    noteScript.free()

    const digestDetails =
      guessedDigests.length > 0
        ? `<ol>${guessedDigests.map((d) => `<li><code>${escapeHtml(d)}</code></li>`).join('')}</ol>`
        : `<p class="hint">
             No standalone 64-hex lines found in the Mast listing. Rust’s
             <code>mast().procedure_digests()</code> is richer than what the JS type exposes.</p>`

    appEl.innerHTML = `
      <main>
        <h1>Note script root</h1>
        <button type="button" id="dl-serialized">Download note.serialized</button>
        <p class="mono root"><code>${escapeHtml(noteRootHex)}</code></p>

        <details>
          <summary>More digests &amp; debug</summary>
          <section>
            <h2>Serialized body SHA-256</h2>
            <p class="hint"><code>NoteScript.serialize()</code> (${serializedForDownload.byteLength} bytes).</p>
            <p class="mono"><code>${escapeHtml(bodySha256)}</code></p>
          </section>
          <section>
            <h2>Procedure digests <span class="hint">(heuristic from Mast dump)</span></h2>
            ${digestDetails}
          </section>
          <details>
            <summary>Full Mast listing</summary>
            <pre class="mast">${escapeHtml(mastText)}</pre>
          </details>
        </details>
      </main>
    `

    document.getElementById('dl-serialized')?.addEventListener('click', () => {
      const blob = new Blob([Uint8Array.from(serializedForDownload)], {
        type: 'application/octet-stream',
      })
      const a = Object.assign(document.createElement('a'), {
        href: URL.createObjectURL(blob),
        download: 'note.serialized',
      })
      a.click()
      URL.revokeObjectURL(a.href)
    })
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    appEl.innerHTML = `<p class="err">Compile failed: ${escapeHtml(msg)}</p>`
  } finally {
    client.terminate()
  }
}

run().catch((e) => {
  const msg = e instanceof Error ? e.message : String(e)
  appEl.innerHTML = `<p class="err">Unexpected error: ${escapeHtml(msg)}</p>`
})
