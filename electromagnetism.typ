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
  #text(size: 13pt)[Matsumotofukashi High School\ 240620 Tsuyoshi Kobayashi]
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

== Mechanism of Electrification


#align(center,box(width:10cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *
      let r = 3
      circle((0,0), radius:r, fill:gray)

      circle((0,0), radius:0.8)
      content((0,0.0), $11+$)

      content((0,-1.8), $10-$)

      circle((4,2), radius:1/8)
      content((4,2.06), "-")

      line((2.5,1.0),(4,2), mark:(end:"stealth", fill:black))

    })
  ]
])


Total amount of charge is saved.

- *Coulomd's law*


#align(center,box(width:6cm, height:3cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let y = -1
      circle((0,y), radius:1/4)
      content((0,y + 0.05), $+$)
      line((1/5,y),(1,y), mark:(end:"stealth", fill:black))
      circle((3,y), radius:1/4)
      content((3,y + 0.05), $-$)
      line((3 - 1/5,y),(3 -1,y), mark:(end:"stealth", fill:black))
      y = 0
      circle((0,y), radius:1/4)
      content((0,y + 0.05), $+$)
      line((1/5,y),(-1,y), mark:(end:"stealth", fill:black))
      circle((3,y), radius:1/4)
      content((3,y + 0.05), $+$)
      line((3 - 1/5,y),(3 +1,y), mark:(end:"stealth", fill:black))
      y = 1
      circle((0,y), radius:1/4)
      content((0,y + 0.05), $-$)
      line((1/5,y),(-1,y), mark:(end:"stealth", fill:black))
      circle((3,y), radius:1/4)
      content((3,y + 0.05), $-$)
      line((3 - 1/5,y),(3 +1,y), mark:(end:"stealth", fill:black))

    })
  ]
])
#align(center,box(width:6cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let x = -2;
      let y = -2;
      circle((x+0,y+0), radius:1/5)
      content((x - 0.2,y+0.6), "charge")
      content((x+0,y - 0.5), $q_1$)
      content((x+1/2+0.3,y+1/2 - 0.1), $F$)
      line((x,y),(x+1,y+1), mark:(end:"stealth", fill:black))

      x = 2;
      y = 2;
      circle((x+0,y+0), radius:1/5)
      content((x+0,y+0.6), "charge")
      content((x+0,y - 0.5), $q_2$)
      content((x - 1/2 - 0.3,y - 1/2 + 0.1), $F$)
      line((x,y),(x - 1,y - 1), mark:(end:"stealth", fill:black))
    })
  ]
])
\

  $
    F = k (q_1 q_2)/ r^2
  $

  $k$ depends on type of surrounding substance.\
  In bacum, $k approx  num("9.0e9") unit("N  m^2/C^2")$


== Electrostatic Indcuction

- *Conductor* - Substances which can conduct electricity.\
  ex) metal, carbon

- *Nonconductor* - Substances which cannot conduct electricity.

- *Semiconductor* - ?????? quite difficult... 中間ってなんだよ

\

- *indcuction*

#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-7.5,-1),(-0.5,1),stroke:(paint: black, thickness:1pt))
      let x = -2;
      let y = -2;

      content((-1,-0.5), $+$)
      content((-1,-0), $+$)
      content((-1,+0.5), $+$)
      content((2.5,-0.5), $+$)
      content((2.7,-0), $+$)
      content((2.5,+0.5), $+$)
      content((1.6,-0.5), $-$)
      content((1.3,-0), $-$)
      content((1.6,+0.5), $-$)
      circle((2,0), radius:1)
      line((1,0),(-0.2,0), mark:(end:"stealth", fill:black))
      line((3,0),(3.5,0), mark:(end:"stealth", fill:black))
      line((2.2,1),(2.6,2))
      content((2,-1.6), "conductor")
    })
  ]
])

- *Dielectric Polarization*

#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-7.5,-1),(-0.5,1),stroke:(paint: black, thickness:1pt))
      rect((0.5,-1),(4,1),stroke:(paint: black, thickness:1pt))

      content((-1,-0.5), $+$)
      content((-1,-0), $+$)
      content((-1,+0.5), $+$)

      circle((1.2,0.5 ),radius:(0.5,0.2))
      circle((1.2,0   ),radius:(0.5,0.2))
      circle((1.2,-0.5),radius:(0.5,0.2))
      content((1.4,-0.5), $+$)
      content((1.4,-0), $+$)
      content((1.4,+0.5), $+$)
      content((1.0,-0.5), $-$)
      content((1.0,-0), $-$)
      content((1.0,+0.5), $-$)

      circle((2.2,0.5 ),radius:(0.5,0.2))
      circle((2.2,0   ),radius:(0.5,0.2))
      circle((2.2,-0.5),radius:(0.5,0.2))
      content((2.4,-0.5), $+$)
      content((2.4,-0), $+$)
      content((2.4,+0.5), $+$)
      content((2.0,-0.5), $-$)
      content((2.0,-0), $-$)
      content((2.0,+0.5), $-$)

      circle((3.2,0.5 ),radius:(0.5,0.2))
      circle((3.2,0   ),radius:(0.5,0.2))
      circle((3.2,-0.5),radius:(0.5,0.2))
      content((3.4,-0.5), $+$)
      content((3.4,-0), $+$)
      content((3.4,+0.5), $+$)
      content((3.0,-0.5), $-$)
      content((3.0,-0), $-$)
      content((3.0,+0.5), $-$)

      content((2.2,-1.6), "nonconductor")

    })
  ]
])

== Electric Field

#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(horizon)[
    #cetz.canvas({
      import cetz.draw: *


      //square(fill: gradient.radial(..color.map.rainbow))
      circle((-1,0),radius:0.2)
      content((-1,-0.7), "charge "+$q_1 unit("C")$)

      line((2,0),(4,0), mark:(end:"stealth", fill:gray), stroke:(paint:gray,thickness:0.1))
      content((4,-0.6), "electric field "+$bold(E(r)) unit("N/C")$)
      /*
      let fieldvec(p,q) = {
        let (x1,y1) = p
        let (x2,y2) = q
        let x3 = x1 - x2
        let y3 = y1 - y2)

        let r = sqrt(x3*x3 + y3*y3)
        let f = 2.0 / (x3*x3 + y3*y3)

        line((2,0),(4,0), mark:(end:"stealth", fill:gray), stroke:(paint:gray,thickness:0.1))
      }
      */
    })
  ]
])

== Electric line of force 電気力線

(it was difficult to draw figure...)

- *the direction of tangent of line is the same as the direction of the field.*
- *the line appears with plus charge and disappear with minus charge.*
- *$E$ line drawn per $1 unit("m^2")$ , as strength of electric field is $E unit("E/C")$.*

#align(center,box(width:8cm, height:8cm, clip:true)[
  #place(horizon)[
    #cetz.canvas({
      import cetz.draw: *


      let l = 4
      let r = 2
      line((0,0,0),(l,0,0), mark: (end: ">", fill:black), name: "x")
      line((0,0,0),(0,l,0), mark: (end: ">", fill:black), name: "y")
      line((0,0,0),(0,0,l), mark: (end: ">", fill:black), name: "z")
      circle((0,0,0), radius:r)
      circle((0,0,0), radius:0.2)
      line((0,-0.4),(r,-0.4), mark: (start: ">",end: ">", fill:black))
      content((r/2,-0.7), $r unit("m")$)
      content((0,0,-0.9), $Q unit("C")$)
      
    })
  ]
])


$
  N = E dot 4 pi r^2
$

Mr.Ide "why integrated value equals to $S$?"\
Me ( Oh.... Does it requires $ε$-$δ$-difinition of limit...? )





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
