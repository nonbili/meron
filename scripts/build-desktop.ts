// Wails v3 delegates builds to Taskfile.yml. Keep Meron's existing output paths,
// platform flags and release metadata so store/signing workflows share one build.
import { mkdir, copyFile, readFile, writeFile, unlink } from 'node:fs/promises'
import { resolve } from 'node:path'

const desktop = resolve(import.meta.dir, '../desktop')
const config = await Bun.file(resolve(desktop, 'wails.json')).json()
const args = process.argv.slice(2)
const dev = args.includes('--dev')
function option(name: string, fallback: string) {
  const index = args.indexOf(name)
  return index < 0 ? fallback : args[index + 1]
}
const host = process.platform === 'win32' ? 'windows' : process.platform
const hostArch = process.arch === 'x64' ? 'amd64' : process.arch
const [os, arch] = option('-platform', `${host}/${hostArch}`).split('/')
const tags = new Set(option('-tags', '').split(/[ ,]+/).filter(Boolean))
if (!dev) tags.add('production')
if (os === 'linux') tags.add('gtk3')
const env = { ...process.env, GOOS: os, GOARCH: arch, CGO_ENABLED: os === 'windows' ? '0' : '1' }
async function run(command: string[], cwd = desktop, extraEnv = env) {
  const child = Bun.spawn(command, { cwd, env: extraEnv, stdout: 'inherit', stderr: 'inherit' })
  const code = await child.exited
  if (code !== 0) throw new Error(`${command.join(' ')} exited ${code}`)
}

await run(['bun', 'install'], resolve(desktop, 'frontend'), process.env)
await run(['bun', 'run', 'build'], resolve(desktop, 'frontend'), process.env)
await mkdir(resolve(desktop, 'build/bin'), { recursive: true })
const filename = option('-o', os === 'windows' ? 'meron.exe' : 'meron')
const binary = resolve(desktop, 'build/bin', filename)

if (os === 'windows') {
  const info = {
    fixed: { file_version: config.info.productVersion },
    info: {
      '0000': {
        ProductVersion: config.info.productVersion,
        CompanyName: config.author.name,
        FileDescription: config.info.productName,
        ProductName: config.info.productName,
        Comments: config.info.comments,
        LegalCopyright: config.author.name,
      },
    },
  }
  await writeFile(resolve(desktop, 'build/windows/info.generated.json'), JSON.stringify(info))
  await writeFile(
    resolve(desktop, 'build/windows/manifest.generated.xml'),
    `<?xml version="1.0" encoding="UTF-8"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1" xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
  <assemblyIdentity type="win32" name="jp.nonbili.meron" version="${config.info.productVersion}.0" processorArchitecture="*"/>
  <dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
  <asmv3:application><asmv3:windowsSettings>
    <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
    <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">permonitorv2,permonitor</dpiAwareness>
  </asmv3:windowsSettings></asmv3:application>
</assembly>`,
  )
  await run(['wails3', 'generate', 'icons', '-input', 'build/appicon.png'], desktop, process.env)
  await run(
    [
      'wails3',
      'generate',
      'syso',
      '-arch',
      arch,
      '-icon',
      'build/windows/icon.ico',
      '-manifest',
      'build/windows/manifest.generated.xml',
      '-info',
      'build/windows/info.generated.json',
      '-out',
      `wails_windows_${arch}.syso`,
    ],
    desktop,
    process.env,
  )
}
async function build(targetArch: string, output: string) {
  await run(
    [
      'go',
      'build',
      '-tags',
      [...tags].join(','),
      '-trimpath',
      '-buildvcs=false',
      ...(dev ? [] : ['-ldflags', os === 'windows' ? '-s -w -H windowsgui' : '-s -w']),
      '-o',
      output,
      '.',
    ],
    desktop,
    { ...env, GOARCH: targetArch },
  )
}
if (arch === 'universal' && os === 'darwin') {
  await build('amd64', `${binary}-amd64`)
  await build('arm64', `${binary}-arm64`)
  await run(['lipo', '-create', '-output', binary, `${binary}-amd64`, `${binary}-arm64`], desktop, process.env)
} else {
  await build(arch, binary)
}
if (os === 'windows') await unlink(resolve(desktop, `wails_windows_${arch}.syso`))
if (os === 'darwin') {
  const bundle = resolve(desktop, 'build/bin/meron.app/Contents')
  await mkdir(`${bundle}/MacOS`, { recursive: true })
  await mkdir(`${bundle}/Resources`, { recursive: true })
  await copyFile(binary, `${bundle}/MacOS/meron`)
  await run(
    ['wails3', 'generate', 'icons', '-input', 'build/appicon.png', '-macfilename', `${bundle}/Resources/iconfile.icns`],
    desktop,
    process.env,
  )
  // Preserve the independent MAS build number from the checked-in template.
  let plist = await readFile(resolve(desktop, 'build/darwin/Info.plist'), 'utf8')
  plist = plist
    .replace(/{{if \.Info\.Protocols}}/g, '')
    .replace(/{{range \.Info\.Protocols}}/g, '')
    .replace(/{{end}}/g, '')
  const values: Record<string, string> = {
    '.Name': 'meron',
    '.OutputFilename': 'meron',
    '.Info.ProductName': config.info.productName,
    '.Info.ProductVersion': config.info.productVersion,
    '.Info.Comments': config.info.comments,
    '.Info.Copyright': 'Nonbili Inc.',
    '.Scheme': 'mailto',
    '.Role': 'Editor',
  }
  plist = plist.replace(/{{\s*([^}]+?)\s*}}/g, (_, key) => {
    if (!(key in values)) throw new Error(`Unknown plist template field: ${key}`)
    return values[key].replaceAll('&', '&amp;').replaceAll('<', '&lt;')
  })
  await writeFile(`${bundle}/Info.plist`, plist)
}
if (args.includes('-nsis')) {
  if (os !== 'windows') throw new Error('-nsis requires a Windows target')
  await run(['wails3', 'generate', 'webview2bootstrapper', '-dir', 'build/windows'], desktop, process.env)
  await run(
    [
      'makensis',
      `-DVERSION=${config.info.productVersion}`,
      `-DARCH=${arch}`,
      resolve(desktop, 'build/windows/installer.nsi'),
    ],
    resolve(desktop, 'build/windows'),
    process.env,
  )
}
