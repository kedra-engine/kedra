// Keep the maintained default theme without distributing bundled font assets.
import DefaultTheme from 'vitepress/theme-without-fonts'
import { useData } from 'vitepress'
import { defineComponent, h } from 'vue'

export default {
  extends: DefaultTheme,
  Layout: defineComponent({
    setup() {
      const { frontmatter } = useData()
      return () => h(DefaultTheme.Layout, null, {
        'doc-footer-before': () => {
          const page = frontmatter.value
          if (!page.apiCrates) return null
          return h('section', { class: 'vp-doc', 'aria-label': page.apiHeading }, [
            h('h2', page.apiHeading),
            h('p', page.apiCrates.length ? page.apiAvailable : page.apiUnavailable),
            page.apiCrates.length ? h('ul', page.apiCrates.map(crate =>
              h('li', { key: crate.name }, [
                // Rustdoc is a separate static site, not a VitePress client route.
                h('a', { href: crate.link, target: '_self' }, crate.name), ` — ${crate.version}`,
              ]))) : null,
          ])
        },
      })
    },
  }),
}
