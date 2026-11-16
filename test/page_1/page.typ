
#import "@preview/cmarker:0.1.10"

// --- INJECTED LAYOUT DEFINITION ---
#import "@preview/cmarker:0.1.10"

#let input_content(content, path) = {
  let resolved_path = if path.starts-with("/") { { path } } else { { "/" + path } }

  if (path.ends-with("md")) {
    if path != "" {
      place(top + left)[
        #block(
          width: 100%,
          height: 100%,
          inset: 12pt,
          cmarker.render(read(path)),
        )
      ]
    }
  } else if (path.ends-with("csv")) {
    if path != "" {
      let bom_data = csv(path)
      let headers = bom_data.at(0)
      let rows = bom_data.slice(1)
      place(top + left)[
        #block(
          width: 100%,
          height: 100%,
          inset: 12pt,
          table(
            columns: headers.len(),
            ..headers.map(h => [*#h*]),
            ..rows.flatten(),
          ),
        )
      ]
    }
  } else {
    if path != "" {
      place(
        top + left,
        image(path, width: 100%, height: 100%, fit: "contain"),
      )
    }
  }
}

#let render_page(layout, local_fields, global_fields, content, path) = {
  set page(
    paper: layout.size,
    flipped: layout.orientation,
    margin: 0mm,
  )

  set text(font: ("Liberation Sans", "Arial", "DejaVu Sans"))

  let frame_width = 100% - 2cm
  let frame_height = 100% - 2cm
  let dx = 1cm
  let dy = 1cm
  if layout.orientation {
    frame_width = 100% - 2cm
    frame_height = 100% - 3cm
    dy = 2cm
  } else {
    frame_width = 100% - 3cm
    frame_height = 100% - 2cm
    dx = 2cm
  }

  let stroke = 2pt + black

  // Outer frame offset by 1cm on top/left, leaving 1cm on bottom/right
  place(top + left, dx: dx, dy: dy)[
    #rect(
      width: frame_width,
      height: frame_height,
      stroke: stroke,
      inset: 0pt,
    )[

      #input_content(content, path)



      // Split of 15 for the rows on the right
      // Split of 5 for the middle column
      #place(bottom + right)[
        #block(
          width: 180mm,
          height: 42mm,
          stroke: stroke,
          inset: 0pt,
          fill: rgb("ffffff"),
          grid(
            columns: (1fr, 1fr, 1fr),
            gutter: 0pt,

            grid(
              rows: (1fr, 3fr),
              gutter: 0pt,
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                inset: 2pt,
                grid(
                  columns: 100%,
                  rows: (auto, 1fr),
                  gutter: 0pt,
                  align(left + top, text(
                    size: 6pt,
                    [Sachnummer, Dateiname:],
                  )),
                  // Centers across the full 100% width and remaining vertical height
                  align(center + horizon, text(
                    size: 10pt,
                    [#local_fields.document_title],
                  )),
                ),
              ),
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                inset: 2pt,
                grid(
                  rows: (2fr, 1fr, 1fr),
                  gutter: 0pt,
                  box(
                    width: 100%,
                    height: 100%,
                    inset: 0pt,
                    align(center + top, image(
                      "logo.png",
                      width: 75%,
                      height: 100%,
                      fit: "contain",
                    )),
                  ),
                  align(center + horizon, text(size: 14pt, weight: 800, [HTBLuVA Salzburg])),
                  align(center + top, text(size: 12pt, weight: 800, [Technische Informatik])),
                ),
              ),
            ),
            grid(
              rows: (1fr, 1fr, 2fr),
              gutter: 0pt,
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                inset: 2pt,
                grid(
                  columns: 100%,
                  rows: (auto, 1fr),
                  gutter: 0pt,
                  align(left + top, text(
                    size: 6pt,
                    [Name:],
                  )),
                  align(center + horizon, text(
                    size: 10pt,
                    [#global_fields.name],
                  )),
                ),
              ),
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                grid(
                  columns: (2fr, 1fr, 1fr, 1fr),
                  gutter: 0pt,
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [ID-Nr.:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.id_nr],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Gez.:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.gez],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Geprüft:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.gepr],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Betreuer:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.betreuer],
                      )),
                    ),
                  ),
                ),
              ),
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                inset: 2pt,
                grid(
                  columns: 100%,
                  rows: (auto, 1fr),
                  gutter: 0pt,
                  align(left + top, text(
                    size: 6pt,
                    [Benennung:],
                  )),
                  // Centers across the full 100% width and remaining vertical height
                  align(center + horizon, box(
                    width: 100%,
                    height: auto,
                    grid(
                      rows: (1fr, 1fr),
                      gutter: 2pt,
                      align(center + bottom, text(
                        size: 16pt,
                        [#global_fields.title],
                      )),
                      align(center + top, text(
                        size: 12pt,
                        [#global_fields.subtitle],
                      )),
                    ),
                  )),
                ),
              ),
            ),
            grid(
              rows: (1fr, 1fr, 1fr, 1fr),
              gutter: 0pt,
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                grid(
                  columns: (6.5fr, 8.5fr),
                  gutter: 0pt,
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Toleranz:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.tolerance],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Werkstoff:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.material],
                      )),
                    ),
                  ),
                ),
              ),
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                grid(
                  columns: (11fr, 4fr),
                  gutter: 0pt,
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Dokumentart:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#local_fields.document_type],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Freigabe:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.share],
                      )),
                    ),
                  ),
                ),
              ),
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                grid(
                  columns: (5fr, 3fr, 7fr),
                  gutter: 0pt,
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Version:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.version],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Revision:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.revision],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Dokumentstatus:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.document_status],
                      )),
                    ),
                  ),
                ),
              ),
              block(
                width: 100%,
                height: 100%,
                stroke: stroke,
                grid(
                  columns: (3fr, 2fr, 6fr, 2fr, 2fr),
                  gutter: 0pt,
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Maßstab:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.scale],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Spr.:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.lang],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Datum:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#global_fields.date],
                      )),
                    ),
                  ),
                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Blatt:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#local_fields.page],
                      )),
                    ),
                  ),

                  block(
                    width: 100%,
                    height: 100%,
                    stroke: stroke,
                    inset: 2pt,
                    grid(
                      columns: 100%,
                      rows: (auto, 1fr),
                      gutter: 0pt,
                      align(left + top, text(
                        size: 6pt,
                        [Blätter:],
                      )),
                      // Centers across the full 100% width and remaining vertical height
                      align(center + horizon, text(
                        size: 10pt,
                        [#local_fields.pages],
                      )),
                    ),
                  ),
                ),
              ),
            ),
          ),
        )
      ]
    ]
  ]
}


// --- INJECTED PROJECT DATA ---
#let project = ("global_fields": ("betreuer": "SreS", "date": "09.09.2026", "document_status": "Prototyp", "gepr": "VacD", "gez": "VacD", "id_nr": "5AHEL", "lang": "DE", "material": "", "name": "David Vacaroiu", "revision": "1", "scale": "1:1", "share": "SreS", "subtitle": "OmniControl", "title": "", "tolerance": "", "version": "1.0"), "pages": (("content": "sch", "layout": ("orientation": true, "size": "a4"), "local_fields": ("document_title": "Beilage", "document_type": "Schaltplan", "page": "1", "pages": "1"), "path": "Venturi_MainPCB_1794348809.svg", "variant": "default"),))
#let global_fields = project.global_fields
#let page_data = project.pages.at(0)

// --- REUSABLE RENDER FUNCTION (for #import "page.typ": render) ---
#let render(
  layout: page_data.layout,
  local_fields: page_data.local_fields,
  global_fields: global_fields,
  content: page_data.content,
  path: page_data.path,
) = {
  render_page(layout, local_fields, global_fields, content, path)
}

// --- DIRECT EXECUTION (for standalone compilation or #include "page.typ") ---
#render()
