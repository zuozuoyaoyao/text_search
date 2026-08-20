// 构建后端 sidecar 并复制到 src-tauri/binaries/<name>-<target-triple>[.exe]
import { execSync } from 'node:child_process'
import { copyFileSync, mkdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const isRelease = process.argv.includes('--release')
const mode = isRelease ? 'release' : 'debug'

console.log(`[build-sidecar] cargo build -p text_search (${mode}) ...`)
execSync(
  `cargo build -p text_search ${isRelease ? '--release' : ''} --features with-ws-server`,
  { cwd: root, stdio: 'inherit' }
)

const rustcInfo = execSync('rustc -vV', { encoding: 'utf8' })
const triple = rustcInfo.match(/host:\s*(\S+)/)?.[1]
if (!triple) throw new Error('cannot detect rustc host triple')

const ext = process.platform === 'win32' ? '.exe' : ''
const src = join(root, 'target', mode, `text_search${ext}`)
const outDir = join(root, 'src-tauri', 'binaries')
mkdirSync(outDir, { recursive: true })
const dest = join(outDir, `text_search-${triple}${ext}`)
copyFileSync(src, dest)
console.log(`[build-sidecar] copied ${src} -> ${dest}`)
