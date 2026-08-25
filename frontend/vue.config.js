module.exports = {
  transpileDependencies: true,
  publicPath: process.env.NODE_ENV === 'production' ? './' : '/',
  pluginOptions: {
    electronBuilder: {
      preload: 'src/preload.js',
      mainProcessFile: 'src/background.js',
      outputDir: 'dist_electron',
      builderOptions: {
        icon: 'resources/icon.png',
        asar: true,
        // 压缩 asar 以减小打包体积，maximum 为最大压缩
        compression: 'maximum',
        extraResources: [
          {
            from: 'resources/backend',
            to: 'backend',
            filter: ['**/*']
          },
          {
            from: 'resources/text-search-electron.desktop',
            to: 'text-search-electron.desktop'
          },
          {
            from: 'resources/icon.png',
            to: 'icon.png'
          }
        ],
        linux: {
          target: ['dir'],
          category: 'Utility',
          icon: 'resources/icon.png'
        },
        win: {
          target: ['dir'],
          icon: 'resources/icon.png'
        },
        afterPack: './scripts/clean-locales.js',
      }
    }
  },
  devServer: {
    port: 8080,
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true,
        pathRewrite: {
          '^/api': ''
        }
      }
    }
  }
}
