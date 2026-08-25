// afterPack hook: 移除多余 locale 文件，只保留 zh-CN
exports.default = async function (context) {
  const fs = require('fs')
  const path = require('path')

  const localesDir = path.join(context.appOutDir, 'locales')
  if (!fs.existsSync(localesDir)) return

  const keep = ['zh-CN.pak']
  let removed = 0

  for (const file of fs.readdirSync(localesDir)) {
    if (!keep.includes(file)) {
      fs.unlinkSync(path.join(localesDir, file))
      removed++
    }
  }

  console.log(`[clean-locales] Removed ${removed} locale files, kept: ${keep.join(', ')}`)
}
