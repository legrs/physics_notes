#import "@preview/physica:0.9.5": *
#import "@preview/unify:0.7.1": *
#import "@preview/cetz:0.4.2"
#import "phy.typ": lined

#set page(
  paper: "a4",
  height: 297mm,
  width: 210mm,
  margin: (x: 1.5cm, y: 1.5cm),
)

// indent
#set par(
  justify: true,
  leading: 1em,
)

#set text(
  font: ("New Computer Modern"),
  size: 15pt,
)

//#show regex("[\p{scx:Han}\p{scx:Hira}\p{scx:Kana}]"): set text(font: "BIZ UDPGothic")
//#set text(lang: "ja")
#show figure.caption: set text(font: ("New Computer Modern"),weight: "bold", size: 12pt)


//#set enum(numbering: "1.",)
#set heading(numbering: "1.1.a ",)
#show heading : set align(center)
#show heading.where(level:1) : set text(size: 30pt)
#show heading.where(level:2) : set text(size: 20pt)
//#show heading : set text(font : "New Computer Modern Uncial")
#set list(marker: [--],)



// タイトル部分
#align(center + horizon)[
  #text(size: 35pt, weight: "bold")[Physics Note]
  #v(0em)
  #text(size: 13pt)[Matsumotofukashi High School 240620 Tsuyoshi Kobayashi]
  #v(0em)
  #text(size: 15pt)[2026-04-14  --]
  #v(1em)
]


#pagebreak()





= _Electric Field_
\
== Electrostatic Force
\
- *electrification* - A process getting charge
- *static electoricity* - static charge
- *electoric charge* - 
- *point charge* - charge which can be ignored the size
\
*Electrostatic force* works between charges.


$
  |bold(F)| = k (q_1 q_2)/ r^2
$

\

== Structure of Atom

\
#align(center,box(width:10cm, height:10cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      circle((0,0), radius:1/3)
      content((0,0.6), "nucleus")
      content((0,0.0), "+")

      circle((3,2), radius:1/8)
      content((3,2.06), "-")
      content((3,2.5), "electron(s)")

      let r = 4
      circle((0,0), radius:r)
      line((-r,-1),(r,-1), mark:(symbol:">", fill:black))
      content((0,-2), $ ~ 10^(-10)  unit("m")$)
    })
  ]
])

- *elementary charge*  $ num("1.6e19") unit("C")$

//2026-04-14




/*

#let angl = 30deg
#let step = 1
#let n = 1.5

#figure(
  align(center,box(width:5cm, height:5cm, clip:true)[
    #place(center + horizon)[
      #cetz.canvas({
        import cetz.draw: *
      
        
        rect((-1,-1),(1,1),stroke:none)

        //centerに影響しない
        floating({
          line((-2.5, 0), (2.5, 0), stroke: (paint: black, thickness: 2pt))
          for i in range(-10,10){
            lined((-i*step/calc.sin(angl), 0), (1, angl), stroke: (paint: black, thickness: 1pt))
            //line((-i*step/calc.sin(angl), 0), (-i*step/calc.sin(angl) + 3/calc.tan(angl), 3), stroke: (paint: black, thickness: 1pt))
            // 1/n
            //line((-i*step/calc.sin(angl), 0), (-i*step/calc.sin(angl) - 3/calc.tan(calc.asin(calc.sin(angl)/n)), -3), stroke: (paint: black, thickness: 1pt))
          }
          content((0.7,0.2), $ i $)
          content((1.0,-0.2), $ r $)
          circle((0,0), radius:0.2)
        })
      })
    ]
  ])
  ,caption: []
)
*/
