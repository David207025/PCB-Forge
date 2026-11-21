#import "layout.typ": render_page

#let project = ("global_fields": ("betreuer": "SreS", "date": "09.09.2026", "document_status": "Prototyp", "gepr": "VacD", "gez": "VacD", "id_nr": "5AHEL", "lang": "DE", "material": "", "name": "David Vacaroiu", "revision": "1", "scale": "1:1", "share": "SreS", "subtitle": "OmniControl", "title": "", "tolerance": "", "version": "1.0"), "pages": (("content": "md", "layout": ("orientation": false, "size": "a4"), "local_fields": ("document_title": "Beilage", "document_type": "Layout Top", "page": "1", "pages": "1"), "path": "test_1794553429.md", "variant": "default"),))
#let global_fields = project.global_fields
#let page_data = project.pages.at(0)

#let render(
  layout: page_data.layout,
  local_fields: page_data.local_fields,
  global_fields: global_fields,
  content: page_data.content,
  path: page_data.path,
) = {
  render_page(layout, local_fields, global_fields, content, path)
}

#render()
