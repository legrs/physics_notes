/*
   Copyright 2026 Igarin & Legrs

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

#import "@preview/physica:0.9.5": *
#import "@preview/unify:0.7.1": *
#import "@preview/cetz:0.4.2"
#import "phy.typ": drawc_t, drawc

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
  font: ("Noto Sans CJK JP"),
  size: 16pt,
)

//#show regex("[\p{scx:Han}\p{scx:Hira}\p{scx:Kana}]"): set text(font: "BIZ UDPGothic")
//#set text(lang: "ja")
#show figure.caption: set text(font: ("New Computer Modern"),weight: "bold", size: 12pt)


//#set enum(numbering: "1.",)
#set heading(numbering: "1.1.a ",)
#set page(numbering: "1")
#set math.equation(numbering:"(1)")
#show heading : set align(center)
#show heading.where(level:1) : set text(size: 30pt,font: ("New Computer Modern"))
#show heading.where(level:2) : set text(size: 20pt,font: ("New Computer Modern"))
#show heading.where(level:3) : set text(size: 17pt,font: ("New Computer Modern"))
//#show heading : set text(font : "New Computer Modern Uncial")
#set list(marker: [--],)



// title部分積分
#align(center + horizon)[
  #text(size: 35pt, weight: "bold",font: ("New Computer Modern"))[Physics Note]
  #v(0em)
  #text(size: 13pt)[Matsumotofukashi High School\ 240620 Tsuyoshi Kobayashi]
  #v(0em)
  #text(size: 15pt)[form 2026-04-14]
  #v(1em)
]


#pagebreak() //どっかーん



//==============================================================================


= _Electric Field_
\
== Electrostatic Force
\
- *electrification* - A process getting charge
- *static electoricity* - static charge
- *electoric charge* - $upright(T I)$
- *point charge* - charge which can be ignored the size
\
*Electrostatic force* is an interraction between charged particles.


$
  |bb(F)| = k (q_1 q_2)/ r^2
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


Total amount of charge is conserved.

- *Coulomb's law*


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

- *Semiconductor* - Substances which have middle electric resistance between conductor and nonconductor.

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

      line((0.9,-0.5),(0.9 - 1,-0.5), mark:(end:"stealth", fill:black))
      line((1.5,-0.5),(1.5 + 0.7,-0.5), mark:(end:"stealth", fill:black))
      line((0.9,0),(0.9 - 1,0), mark:(end:"stealth", fill:black))
      line((1.5,0),(1.5 + 0.7,0), mark:(end:"stealth", fill:black))
      line((0.9,0.5),(0.9 - 1,0.5), mark:(end:"stealth", fill:black))
      line((1.5,0.5),(1.5 + 0.7,0.5), mark:(end:"stealth", fill:black))

      content((2.2,-1.6), "nonconductor")

    })
  ]
])

#pagebreak()

== Electric Field

\

#align(center,box(width:15cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *
      rect((-10,-10),(10,10))


      let charge_pos = (0,0)
      circle(charge_pos,radius:0.3)
      content((0,0.05), $-$)
      content((-1.8,-0.2), "charge "+$q_1 unit("C")$)

      line((2,0.2),(4,0.2), mark:(end:"stealth", fill:gray), stroke:(paint:gray,thickness:0.1))
      content((4,-0.2), "electric field "+$bb(E(r)) unit("N/C")$)
    

      let fieldvec(p,q) = {
        let (x1,y1) = p
        let (x2,y2) = q
        let x3 = x1 - x2
        let y3 = y1 - y2

        let f = 0
        let r = 1
        if x3 == 0 and y3 == 0{

        }else{
          r = calc.sqrt(x3*x3 + y3*y3)
          f = 2 / (x3*x3 + y3*y3)
        }

        line(q,(x2 + x3*f/r,y2 + y3*f/r), mark:(end:"stealth", fill:black), stroke:(paint:black,thickness:0.01))
      }
      for i in range(-6,6){
        for j in range(-8,8){
          if calc.abs(i) <= 1 or calc.abs(j) <= 1{
          }else{
            fieldvec(charge_pos,(i/2,j/2))
          }
        }
      }
      //fieldvec(charge_pos,(0,0))
    })
  ]
])

Electric field is vector field.\
Let $1 unit("C")$ charge be a test charge at position $bb(r)$,
$bb(E(r))$ is the electric force which the test charge receive.
$
  bb(E(r)) = k Q bb(r)/abs(bb(r))^3
$

If there are multiple charges, the electric field is equals to sum of each electric fields made by these charges.

#pagebreak()

== Electric line of force

#figure(
align(center,box(width:10cm, height:8cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let q1 = +1
      let q2 = -2
      let d = 4
      let k = 0.5

      rect((-100 + d/2 ,-100),(100 + d/2,100))

      circle((0,0),radius:0.5)
      //content((0,0), str(q1)+" C", )
      content((0,0), text(size:9pt,weight:900)[#str(q1) C])

      circle((d,0),radius:0.5)
      content((d,0), text(size:9pt,weight:900)[#str(q2) C])

      let drawel(p,angl,doreverse) = {
        let (x,y) = p
        x += 0.5 * calc.cos(angl)
        y += 0.5 * calc.sin(angl)
        let end = false
        let flag1 = true
        let flag2 = true
        let i = 0
        while not end{
          let r1 = calc.sqrt(x*x + y*y)
          let x_p = x - d 
          let y_p = y 
          let r2 = calc.sqrt(x_p*x_p + y_p*y_p)
          let r1_3 = calc.pow(r1,3)
          let r2_3 = calc.pow(r2,3)
          let fx = k * (q1 * x / r1_3 + q2 * x_p / r2_3)
          let fy = k * (q1 * y / r1_3 + q2 * y_p / r2_3)
          if r1 <= 0.5 or r2 <= 0.5{
            let ff = calc.sqrt(fx*fx + fy*fy)
            if 0.7 <= ff {
              let k_p = 0.7/ff
              fx = k_p * fx
              fy = k_p * fy
            }
          }
          if doreverse and q2 < 0{
            fx = -fx
            fy = -fy
          }
          //let fx = x_p / r2
          //let fy = y_p / r2
          if x <= d/2 - 5 or d/2 + 5 <= x{
          //if (not doreverse and d/2 <= x) or (doreverse and x <= d/2){
            end = true
          }else if y <= -4 or 4 <= y{
            end = true
          }else if (x <= d and d <= x+fx) or (y <= 0 and 0 <= y+fy){
            fx = x_p * 0.5 / r2
            fy = y_p * 0.5 / r2

            line((x,y),(d + fx, 0 + fy))
            end = true
          }else if 1000 < i{
            end = true
          }else{

            if doreverse{
              if flag1 and d/2 <= r2  {
                line((x,y),(x + fx, y + fy), mark:(end:"<", fill:black), stroke:(thickness:0.05))
                flag1 = false
              }else if flag2 and d/2 >= r1{
                line((x,y),(x + fx, y + fy), mark:(end:"<", fill:black), stroke:(thickness:0.05))
                flag2 = false
              }else{
                line((x,y),(x + fx, y + fy))
              }
            }else{

              if flag1 and d/2 <= r1  {
                line((x,y),(x + fx, y + fy), mark:(end:">", fill:black), stroke:(thickness:0.05))
                flag1 = false
              }else if flag2 and d/2 >= r2{
                line((x,y),(x + fx, y + fy), mark:(end:">", fill:black), stroke:(thickness:0.05))
                flag2 = false
              }else{
                line((x,y),(x + fx, y + fy))
              }
            }
          }
          x += fx
          y += fy
          i += 1
        }
      }
      drawel((0,0),0, false)
      drawel((0,0),calc.pi * 1 / 4, false)
      drawel((0,0),calc.pi * 2 / 4, false)
      drawel((0,0),calc.pi * 3 / 4, false)
      drawel((0,0),calc.pi * 4 / 4, false)
      drawel((0,0),calc.pi * 5 / 4, false)
      drawel((0,0),calc.pi * 6 / 4, false)
      drawel((0,0),calc.pi * 7 / 4, false)
      //drawel((d,0),calc.pi * 1 / 4, true)
      //drawel((d,0),calc.pi * 2 / 4, true)
      //drawel((d,0),calc.pi * 3 / 4, true)
      //drawel((d,0),calc.pi * 5 / 4, true)
      //drawel((d,0),calc.pi * 6 / 4, true)
      //drawel((d,0),calc.pi * 7 / 4, true)
      drawel((d,0),calc.pi * 8 / 4, true)
    })
  ]
])
  ,caption: [simulator]
)
(it was difficult to draw the figure...)

- *the direction of tangent of line is the same as the direction of the field's vector.* 
- *the line appears with plus charge and disappear with minus charge.*
- *$E$ lines drawn per $1 unit("m^2")$ , as strength of electric field is $E unit("N/C")$.*

#align(center,box(width:8cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *


      let l = 3
      let r = 1.5
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

Let $N$ be number of lines,$Q$ amount of charge.\
$N = E dot 4 pi r^2 =  k Q/r^2 dot 4 pi r^2$\
Thus,
$
  N = 4 pi k Q
$

#pagebreak()

== Electric Potential
\
Let $U$ potential energy of charge which have $q$, $V$ electric potential,
$
  V := U / q
$
Thus, $V$ is potential energy per unit charge.

#align(center,box(width:15cm, height:2cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      circle((0,0),radius:0.2)
      content((0,0.05), $+$)
      
      circle((2,0),radius:0.05, fill:black)
      line((2,-0.2),(0,-0.2), mark:(start:">",end:">",fill:black))
      content((1,-0.5),$r$)
      content((0,-0.5),$Q$)
    })
  ]
])
Let infinity be reference level of potential,

$V(r) = integral_r^infinity d r dot k Q / r^2 = k Q [ -r^(-1) ]_r^infinity = k Q / r$
$
  bold(V(r) = k Q / r)
$
\



#align(center,box(width:15cm, height:2cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-1,-0.5),(1,0.5))
      rect((1,0.25),(1.15,-0.25))
      content((0,0), text(size:10pt)[BATTERY])

      content((1.6,-0.4), $V_("A")$)
      content((-1.5,-0.4), $V_("B")$)
    })
  ]
])
$V_("AB") = V_A - V_B $  (reference is $B$)\
$V_("AB")$ is also called *voltage*$[upright(M L^2 T^(-3) I^(-1))]$.


#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      circle((0,2),radius:0.3)
      content((0,2.05), "+")
      content((0.7,2), $q$)
      line((0,2),(0,0), mark:(end:">", fill:black))
      content((3.4,1), "electric force (constant)")
      line((-1,-1),(1,-1))
      content((3,-1), "reference level of potential")
    })
  ]
])
$
  W_("AB") = q(V_A - V_B)
$\

== Relative of Field and Potential

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      circle((0.2,1),radius:0.25)
      content((0.2,1.05), "+")
      content((0.6,1.3), $q$)
      line((0,2),(0,0), mark:(end:">", fill:black))
      line((0,0),(0,-2))
      line((-0.9,2),(-0.9,0), mark:(end:">", fill:black))
      line((-0.9,0),(-0.9,-2))
      line((0.9,2),(0.9,0), mark:(end:">", fill:black))
      line((0.2,1),(0.2,-0.2), mark:(end:">", fill:black), stroke:(thickness:0.08))
      content((0.6,0.5), $bold(q E)$)
      line((0.9,0),(0.9,-2))
      content((4.2,1.5), [*uniform electric field* $bold(E)$])
      line((-1,-2),(1,-2))
      line((-1,2),(1,2))
      content((-0.9,-2.2), $-$)
      content((0,-2.2), $-$)
      content((+0.9,-2.2), $-$)
      content((-0.9,+2.2), $+$)
      content((0   ,+2.2), $+$)
      content((+0.9,+2.2), $+$)
      line((1.5,1.05),(1.5,-2), mark:(start:">",end:">", fill:black))
      content((2.8,-0.4), "distance "+$d$)
    })
  ]
])

$
q E d = q V\
  <=> V = E d
$

* electric potential is proportional to energy, so it's superposition is calculated by sum.*


== Equipotential Surface


#figure(
align(center,box(width:10cm, height:7cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let q1 = +1
      let q2 = -2
      let d = 4
      let k = 5

      rect((-100 + d/2 ,-100),(100 + d/2,100))

      circle((0,0),radius:0.5)
      content((0,0), text(size:9pt,weight:900)[#str(q1) C])

      //circle((d,0),radius:0.5)
      //content((d,0), text(size:9pt,weight:900)[#str(q2) C])

      let drawel(p,m) = {
        let (x,y) = p
        circle(p,radius:k*q1/m)
      }
      drawel((0,0),1)
      drawel((0,0),2)
      drawel((0,0),3)
      drawel((0,0),4)
      drawel((0,0),5)
      drawel((0,0),6)
      drawel((0,0),7)
      drawel((0,0),8)
      drawel((0,0),9)
    })
  ]
])
  ,caption: [simulator]
)

- *Equiipotential Surface*  -  A surface which was made by joyning points that have equal electric potential.

//0425
* equipotential surface is perpendicular to electric line of force.*\
( if test charge move with a direction perpendicular to electric force , work is $W = bb(F dot Delta x) = 0$ $<=>$ potential is same constantly. )
$
  1/2 m v^2 + q V = "const."
$

#pagebreak()

== Substance and Electric Field

=== Conductor

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-1,1.6),(1,-1.9))
      line((-2,2),(2,2), mark:(end:">", fill:black))
      line((-2,1),(2,1), mark:(end:">", fill:black))
      line((-2,0),(2,0), mark:(end:">", fill:black))
      line((-2,-1),(2,-1), mark:(end:">", fill:black))
      line((-2,-2),(2,-2), mark:(end:">", fill:black))
      content((-0.8,1.3), $-$)
      content((-0.8,0.3), $-$)
      content((-0.8,-0.7), $-$)
      content((-0.8,-1.6), $-$)
      content((0.8,1.3), $+$)
      content((0.8,0.3), $+$)
      content((0.8,-0.7), $+$)
      content((0.8,-1.6), $+$)
      line((0.5,1.3),(-0.5,1.3), mark:(end:">", fill:black))
      line((0.5,0.3),(-0.5,0.3), mark:(end:">", fill:black))
      line((0.5,-0.7),(-0.5,-0.7), mark:(end:">", fill:black))
      line((0.5,-1.6),(-0.5,-1.6), mark:(end:">", fill:black))
    })
  ]
])

#align(center,box(width:15cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-1000,-1000),(1000,1000))
      let r = 2 //radius of conductor
      let d = 0.8  //step
      let f = 0.4 // force
      let k = -calc.floor(r/d)
      let minus = ()
      let plus = ()
      
      circle((0,0),radius:r)
      while k*d <= r{
        minus.push((-calc.sqrt(r*r - k*k*d*d),k*d))
        plus.push((calc.sqrt(r*r - k*k*d*d),k*d))
        //line((-15/2,k*d),(15/2,k*d), mark:(end:">", fill:black))
        content((-calc.sqrt(r*r - k*k*d*d) + 0.2,k*d + 0.05),$-$)
        content((calc.sqrt(r*r - k*k*d*d) - 0.2,k*d + 0.05),$+$)
        k += d
      }

      let k = -calc.floor(r/d) - 2
      let fac = 0.05
      while k < 0{
        let x = -5
        let y = k*d

        let flag = true

        let i = 0
        while flag{
          let fx = f
          let fy = 0
          for i in minus{
            let (mx,my) = i
            let dx = mx - x
            let dy = my - y
            let a = calc.sqrt(dx*dx + dy*dy)
            if fac / (a*a) < 1{
              fx += fac * dx / (a*a*a)
              fy += fac * dy / (a*a*a)
            }
          }
          for i in plus{
            let (mx,my) = i
            let dx = mx - x
            let dy = my - y
            let a = calc.sqrt(dx*dx + dy*dy)
            if fac / (a*a) < 1{
              fx -= fac * dx / (a*a*a)
              fy -= fac * dy / (a*a*a)
            }
          }
          if calc.sqrt(calc.pow(x+fx,2)+calc.pow(y+fy,2)) <= r{
            flag = false
            let dis = calc.sqrt(calc.pow(x,2)+calc.pow(y,2))
            if dis==0{
            }else{
              line((x,y),(x*r/dis,y*r/dis))
              line((x,-y),(x*r/dis,-y*r/dis))
              line((-x,y),(-x*r/dis,y*r/dis))
              line((-x,-y),(-x*r/dis,-y*r/dis))
            }
          }else if 0 <= x+fx {
            flag = false
            line((x,y),(0,y), mark:(end:">", fill:black))
            line((x,-y),(0,-y), mark:(end:">", fill:black))
            line((-x,y),(0,y))
            line((-x,-y),(0,-y))
          }else{
            //line((x,y),(x+fx,y+fy), mark:(end:">", fill:black))
            if x < -5/2 and -5/2 < x+fx{
              line((x,y),(x+fx,y+fy), mark:(end:">", fill:black))
              line((x,-y),(x+fx,-(y+fy)), mark:(end:">", fill:black))
              line((-x,y),(-(x+fx),y+fy), mark:(end:"<", fill:black))
              line((-x,-y),(-(x+fx),-(y+fy)), mark:(end:"<", fill:black))
            }else{
              line((x,y),(x+fx,y+fy))
              line((x,-y),(x+fx,-(y+fy)))
              line((-x,y),(-(x+fx),y+fy))
              line((-x,-y),(-(x+fx),-(y+fy)))
            }
            x += fx
            y += fy
          }
          i += 1
          if 100 < i{
            flag = false
          }
        }
        k += d
      }

      content((0,0),[*conductor*])
      
    })
  ]
])

*In conductor, the electric field is $bb(0)$. *
(charges contiune moving while field is not $bb(0)$,)

It means that electric potential is the same in metal.

- *Electric field is perpendicular to surface of metal.* (metal distorts surrounding electric field)
- *Electric charges do not appear in internal of metal.* (on surface only)
#text(size:10pt)[( why? is not impossible to make field $bb(0)$ with it having charges in metal? そうかな…そうかも…引力と斥力があるから自動的に表面にでできるかね。べつに全電荷が打ち消しに貢献する必要はなく、正負がsurfaceに交互に並んでいればいい？ )]

- *Electric Shielding * - In cavities in conductor, electric field is $~bb(0)$. It means field in the cavity is not affected by the outside field.
#text(size:10pt)[( これマジでなんで？？)]

#pagebreak()

=== Nonconductor

*In nonconductor, the electric field is smaller thant the ouside field.*
(due to the electric field generated by dielectric polarization)

== Capacitor

\
#figure(
align(center,box(width:15cm, height:1cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      line((0,0.1),(4,0.1))
      line((0,-0.1),(4,-0.1))
      
      content((0.5,0.4), $+$)
      content((1.5,0.4), $+$)
      content((2.5,0.4), $+$)
      content((3.5,0.4), $+$)
      content((0.5,-0.4), $-$)
      content((1.5,-0.4), $-$)
      content((2.5,-0.4), $-$)
      content((3.5,-0.4), $-$)
    })
  ]
])
,caption:[parallel-plate capacitor]
)
#figure(
align(center,box(width:15cm, height:3cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      line((-2,1),(0,1))

      line((0,1),(0.6,1.05))

      line((0.6,1),(2,1))
      line((2,1),(2,0))

      line((1.5,0),(2.5,0))
      line((1.5,-0.2),(2.5,-0.2))

      line((2,-0.2),(2,-1.5))
      line((2,-1.5),(-2,-1.5))
      line((-2,-1.5),(-2,-0.3))

      line((-2.5,0),(-1.5,0))
      line((-2.2,-0.3),(-1.8,-0.3))

      line((-2,0),(-2,1))
      content((-2.8,0),$bold(V)$)

      line((2.2,0.2),(2.2,1), mark:(end:">",fill:black))
      content((2.6,0.6),$e^-$)
      line((2.2,-1.3),(2.2,-0.4), mark:(end:">",fill:black))
      content((2.6,-0.6),$e^-$)

      content((2.9,-0.1),$bold(C)$)
      content((1.1,0.2),$bold(+Q)$)
      content((1.1,-0.4),$bold(-Q)$)
    })
  ]
])
,caption:[charging]
)

\
$
  bold( C := Q / V )
$

$C$ is called *electric capacity* $[upright(M^(-1) L^(-2) T^(-2) I^2)]$

#text(size:10pt)[なにこに次元は…電磁気の次元解析ってけっこう魔境…？独自文字で置いてやろうか]

* Parallel-plate capacitor makes uniform electric field. *\
It makes $4 pi k Q "lines of electric force"$ .

#text(size:10pt)[( 電気力線ってsuperpositionできるの？uniformであることは積分で証明？)]

#figure(
align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      content((-5,0), $+$)
      content((-4,0), $+$)
      content((-3,0), $+$)

      let draw_uni(p)={
        let (x,y) = p
        line(p,(x - 1   ,y + 1), mark:(end:">", fill:black))
        line(p,(x - 1.41,y + 0), mark:(end:">", fill:black))
        line(p,(x + 1.41,y + 0), mark:(end:">", fill:black))
        line(p,(x + 0   ,y - 1.41), mark:(end:">", fill:black))
        line(p,(x - 1   ,y - 1), mark:(end:">", fill:black))
        line(p,(x + 1   ,y - 1), mark:(end:">", fill:black))
        line(p,(x + 0   ,y + 1.41), mark:(end:">", fill:black))
        line(p,(x + 1   ,y + 1), mark:(end:">", fill:black))
      }

      draw_uni((-5,0))
      draw_uni((-4,0))
      draw_uni((-3,0))

      line((-0.5,0),(0.5,0), mark:(end:">", fill:gray), stroke:(thickness:0.3, paint:gray))

      content((5,0), $+$)
      content((4,0), $+$)
      content((3,0), $+$)

      line((3,0),(3 - 1.41,0), mark:(end:">", fill:black))
      line((3,0),(3 - 1,1), mark:(end:">", fill:black))
      line((3,0),(3 - 1,-1), mark:(end:">", fill:black))
      line((3,0),(3 - 1,1), mark:(end:">", fill:black))
      line((3,0),(3,1.41), mark:(end:">", fill:black))
      line((3.33,0),(3.33,1.41), mark:(end:">", fill:black))
      line((3.66,0),(3.66,1.41), mark:(end:">", fill:black))
      line((4,0),(4,1.41), mark:(end:">", fill:black))
      line((4.33,0),(4.33,1.41), mark:(end:">", fill:black))
      line((4.66,0),(4.66,1.41), mark:(end:">", fill:black))
      line((5,0),(5,1.41), mark:(end:">", fill:black))
      line((3,0),(3,-1.41), mark:(end:">", fill:black))
      line((3.33,0),(3.33,-1.41), mark:(end:">", fill:black))
      line((3.66,0),(3.66,-1.41), mark:(end:">", fill:black))
      line((4,0),(4,-1.41), mark:(end:">", fill:black))
      line((4.33,0),(4.33,-1.41), mark:(end:">", fill:black))
      line((4.66,0),(4.66,-1.41), mark:(end:">", fill:black))
      line((5,0),(5,-1.41), mark:(end:">", fill:black))
      line((5,0),(5 + 1.41,0), mark:(end:">", fill:black))
      line((5,0),(5 + 1,1), mark:(end:">", fill:black))
      line((5,0),(5 + 1,-1), mark:(end:">", fill:black))
    })
  ]
])
,caption:[superposition of electric field which made by lined charges]
)

#pagebreak()

Let \
surface area of one side plate $bold(S)$,\
amount of electric charge $bold(Q)$,\
distance between plates $bold(d)$

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      line((-4,2),(4,2))
      line((-4,-2),(4,-2))
      
      for i in range(4){
        content((i,2.2), $+$)
        content((-i,2.2), $+$)
        content((i,-2.2), $-$)
        content((-i,-2.2), $-$)
        line((i,2),(i,-2), mark:(end:">", fill:black))
        line((-i,2),(-i,-2), mark:(end:">", fill:black))
      }

      line((5,2),(5,-2), mark:(start:">",end:">", fill:black))
      content((4.5,2), $bold(S)$)
      content((3.8,2.3), $bold(+Q)$)
      content((8,0), [number of lines $4 pi k Q$])
      content((4.6,0), $bold(d)$)
      content((7,-1), [$ E = (4 pi k Q )/ S$])

    })
  ]
])
\
$V = E d = (4 pi k Q)/ S d\
Q = 1 / (4 pi k) dot S/d dot V\
$
$
  bold(therefore C = 1/(4 pi k) dot S/d)
$

Electric capacity depends on $k$ , $S$ ,and $d$.

Where, let $epsilon = 1/(4 pi k)$ ,
$epsilon$ is called permitivity $[upright(M^(-1) L^(-3) T^4 I^2)]$.
$
  C = epsilon dot S / d
$
In a lower permittivity substance, strength of the  electric field is smaller.
From $V = k dot Q / r$, 
even with the same voltage, the amount of charge is greater.

dielectric constant : $ epsilon_r = epsilon / epsilon_0$

#pagebreak()

=== Series Connection

#align(center,box(width:15cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      line((0,3),(0,2))
      line((-1,2),(1,2))
      line((-1,1),(1,1))
      line((0,1),(0,-1))
      line((-1,-1),(1,-1))
      line((-1,-2),(1,-2))
      line((0,-2),(0,-3))

      content((-0.1,0), highlight(fill:white)[　])
      content((-0.1,0), $C$)

      content((-1,2.5), $bold(+ Q)$)
      content((-1,-2.5), $bold(- Q)$)

      content((0,1.5), $C_1$)
      content((0,-1.5), $C_2$)

      line((2.5,-3),(2.5,3), mark:(start:">",end:">", fill:black))
      content((2.8,0), $V$)
      line((1.5,0),(1.5,3), mark:(start:">",end:">", fill:black))
      line((1.5,0),(1.5,-3), mark:(start:">",end:">", fill:black))
      content((1.8,1.5), $V_1$)
      content((1.8,-1.5), $V_2$)

      rect((-1.9,1.2),(1.4,-1.2), stroke:(paint:red))
      content((-3.6,0), text(fill:red)[#highlight(fill:white)[amount of charge 0]])

      /*
      line((-4,2),(4,2))
      line((-4,-2),(4,-2))
      
      for i in range(4){
        content((i,2.2), $+$)
        content((-i,2.2), $+$)
        content((i,-2.2), $-$)
        content((-i,-2.2), $-$)
        line((i,2),(i,-2), mark:(end:">", fill:black))
        line((-i,2),(-i,-2), mark:(end:">", fill:black))
      }

      line((5,2),(5,-2), mark:(start:">",end:">", fill:black))
      content((4.5,2), $bold(S)$)
      content((3.8,2.3), $bold(+Q)$)
      content((8,0), [number of lines $4 pi k Q$])
      content((4.6,0), $bold(d)$)
      content((7,-1), [$ E = (4 pi k Q )/ S$])
      */

    })
  ]
])

The amount of charge is $0$, because one side plate of each is connected by the conductor.

$V = V_1 + V_2$

$Q / C = Q / C_1 + Q / C_2$
$(because V = Q / C)$

$
  bold(therefore C^(-1) = C_1^(-1) + C_2^(-1))
$

- V is proportional to $C^(-1)$
//$ C = (C_1 C_2)/ (C_1 + C_2)$

\

=== Energy of capacitor

#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      line((0,2),(0,1))
      line((-1,1),(1,1))
      line((-1,-1),(1,-1))
      line((0,-1),(0,-2))

      content((0.5,1.5), $bold(+ Q)$)
      content((0.5,-1.5), $bold(- Q)$)

      line((-1.0,1),(-1.0,-1), mark:(start:">",end:">", fill:black))
      content((-0.1,0), $bold(V(Q))$)
      //content((0,0), $bold(C)$)

      line((1.5,0),(2.5,0), mark:(end:">", fill:gray), stroke:(paint:gray, thickness:0.2))
      //line((-0.5,0),(0.5,0), mark:(end:">", fill:gray), stroke:(thickness:0.3, paint:gray))
      
      line((4,2),(4,1))
      line((3,1),(5,1))
      line((3,-1),(5,-1))
      line((4,-1),(4,-2))

      content((5.0,1.5), $bold(+Q - q)$)
      content((4.5,-1.5), $bold(- Q)$)

      line((3.0,1),(3.0,-1), mark:(start:">",end:">", fill:black))
      content((3.8,0), $bold(V(q))$)

      line((5,0.8),(5,0.4), mark:(fill:black), stroke:(thickness:0.02))
      line((5,-0.4),(5,-0.8), mark:(end:">", fill:black), stroke:(thickness:0.02))
      content((5,0), $d q$)

      line((5.5,0),(6.5,0), mark:(end:">", fill:gray), stroke:(paint:gray, thickness:0.2))
      
      line((8,2),(8,1))
      line((7,1),(9,1))
      line((7,-1),(9,-1))
      line((8,-1),(8,-2))

      content((8,0), $bold(0)$)
    })
  ]
])
$ V(q) = q/C \
W = integral_0^Q d q dot V(q) = integral_0^Q d q dot q/C= 1/2 Q^2 /C = 1/2 Q V
$
$
therefore bold(W = 1/2 Q V)
$

* The work which battery has done :$W = Q V$. ( V=const. )*
Therefoe Work:$1/2 Q V$ is wasted.

#figure(
align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *
/*
      line((0.1,0,0),(1,0,0))
      line((1,0,0),(1,2,1))
      line((1,2,1),(0.1,2,1))
      line((-0.1,2,0),(-1,2,0))
      line((-1,2,0),(-1,0,0))
      line((-1,0,0),(0,0,0))

      line((0,0,0),(3,0,0), mark:(end:">", fill:black))
      line((0,0,0),(0,3,0), mark:(end:">", fill:black))
      line((0,0,0),(0,0,2), mark:(end:">", fill:black))
      */

      ortho(x:30deg, y:150deg,{
        let AXIS_LENGTH = 2.5
        //line((0, 0, 0), (AXIS_LENGTH, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
        //line((0, 0, 0), (0, AXIS_LENGTH, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
        //line((0, 0, 0), (0, 0, AXIS_LENGTH), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
        //content("x-axis.end", [$x$], anchor: "west")
        //content("y-axis.end", [$y$], anchor: "south")
        //content("z-axis.end", [$z$], anchor: "north-east")

        line((0.1,0,-1),(2,0,-1))
        line((2,0,-1),(2,1,2))
        line((2,1,2),(0.1,1,2))
        //line((2,0,-1),(2,0,0.5), mark:(end:">",fill:black))
        //line((2,0,-1),(0.5,0,-1), mark:(end:">",fill:black))
        //line((2,0,-1),(2,1.5,-1), mark:(end:">",fill:black))
        line((-0.1,0,2),(-1,0,2))
        line((-1,0,2),(-1,0,-1))
        line((-1,0,-1),(-0.1,0,-1))

        line((0.1,0,-1.5),(0.1,0,-0.5))
        line((-0.1,0,-1.5),(-0.1,0,-0.5))

        line((0.1,1,2.3),(0.1,1,1.7))
        line((-0.1,0,2.5),(-0.1,0,1.5))

        line((2.2,1,2),(2.2,0,-1), mark:(start:">",fill:black))
        content((2.5,0.5,1), $bold(e^-)$)
      })
    })
  ]
])
,caption:[The start of charging]
)

#pagebreak()


//==============================================================================



= _Electric Current_
\
== Current
\
*Electric Current*$"[I]"$ - A flow of charged particles. 

- *Direct Current* - a current that is one-directional
- *Alternating Current* - a current that periodically reverses direction

\

I : current (constant),
Q : amount of charge,
t : time
$
  I := Q / t
$<eq_i>
$unit("A") = unit("C/s")$


== Ohm's law

//法則だから、これは受け入れるしか無い、と思ったら大間違い。
//ちゃんと意味があるから、オーム先生のことをじっくり考えよう。

Let elementary charge $e$
, speed of charges $v$
, number of charges per unit volume $n$
, surface of wire $S$
, 

#align(center,box(width:15cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      ortho(x:0deg, y:-80deg,{
        circle((0,0,0),radius:2)
        circle((0,0,10),radius:2)
        line((0,2,0),(0,2,10))
        line((0,-2,0),(0,-2,10))

        line((1,0,5),(1,0,2), mark:(end:">",fill:black),stroke:(thickness:0.05))
        content((1,0,5.9),$bold(e dot n)$)
        content((1,-0.5,3.5),$bold(v)$)

        let AXIS_LENGTH = 2.5
        //line((0, 0, 0), (AXIS_LENGTH, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
        //line((0, 0, 0), (0, AXIS_LENGTH, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
        //line((0, 0, 0), (0, 0, AXIS_LENGTH), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
        //content("x-axis.end", [$x$], anchor: "west")
        //content("y-axis.end", [$y$], anchor: "south")
        //content("z-axis.end", [$z$], anchor: "north-east")
      })
      circle((6,0),radius:1)

      content((0,0),$bold(S)$)
    })
  ]
])
$Q = e dot n dot v t dot S\
therefore I = e n v S 
$
#align(center,box(width:15cm, height:7cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      ortho(x:0deg, y:-80deg,{
        circle((0,0,0),radius:2)
        circle((0,0,10),radius:2)
        line((0,2,0),(0,2,10))
        line((0,-2,0),(0,-2,10))

        line((1,0.2,6),(1,0.2,2), mark:(end:">",fill:black),stroke:(thickness:0.06))
        line((1,0,6),(1,0,4), mark:(end:">",fill:black))
        line((1,0,6),(1,0,8), mark:(end:">",fill:black))

        let AXIS_LENGTH = 2.5
        //line((0, 0, 0), (AXIS_LENGTH, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
        //line((0, 0, 0), (0, AXIS_LENGTH, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
        //line((0, 0, 0), (0, 0, AXIS_LENGTH), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
        //content("x-axis.end", [$x$], anchor: "west")
        //content("y-axis.end", [$y$], anchor: "south")
        //content("z-axis.end", [$z$], anchor: "north-east")
      })
      circle((6,0),radius:0.5,fill:white)
      content((6,0),$bold(e)$)
      content((4,0.5),$bold(v)="const."$)

      content((4.8,-0.5),$bold(e dot V/l)$)
      content((7.2,-0.4),$bold(k v)$)

      content((0,1),$bold(S)$)
      line((0,0),(-1,0))
      line((-1,0),(-1,-4))
      line((-1,-4),(4.8,-4))
      line((5.2,-4),(11,-4))
      line((11,-4),(11,0))
      line((11,0),(10,0))

      line((4.8,-3),(4.8,-5))
      line((5.2,-3.5),(5.2,-4.5))
      content((5,-2.8),$bold(V)$)

      line((0,-1.7),(10,-1.7),mark:(start:">",end:">", fill:black))
      content((5,-1.7),text(fill:white)[#highlight(fill:white)[　]])
      content((5,-1.7),$bold(l)$)
    })
  ]
])
There shoud be force $bold(k v) ( bold(k)="const." )$ which against electric force $bold(e dot V/l)$,
because velocity of electron $bold(v) = "const."$ 


#text(size:10pt)[ ※ energyが低い$eq.not$安定]

#h(2em)
$bold(k v = e V/l) \
<=> v = e V/(l k) quad ( v prop E )\
<=> I = e n dot e V/(l k) dot S quad ( because I = e n v S ) \
<=> I = e^2 n S / (k l) dot V  \ 
$

Let $ k l / (e^2 n S)$ as $R$,

#h(2em)
$I = V / R
$


$
  therefore bold(V = R I)
$
V : a.k.a voltage dump

\
// 0529 物理はすすめずに複素数やりました．

$R = 1/(e^2 n) dot l/S wide$ Let $1/(e^2 n)$ as $bold(rho)$, 
$
  bold(R = rho l/S)
$

$rho$ is called *resistivity* and depends on substances. ( resistivity of alminium is $tilde num("2.7e-8")$ )


#text(size:10pt)[ちうがくせいのころよく分からなかったのが解消されていってたのしー！！]

#pagebreak()

#text(size:10pt)[ いまさらだが保存力ってけっこう難しい．fieldを解析してどうなったら保存力なんだろう．位置にのみ依存することが必要条件になってはいそう．十分条件はなんなの　直感的な理解が足りていません。百足らず様がお通りになられたせいでしょうか。]

*Resistivity also depends on templature  due to thermal motion.*

This is related to ratio of electron's moving speed to poisitive ion's moiton speed.

#text(size:10pt)[ constantな電圧をかけたら，抵抗値は時間発展するの？する？]

$
  bold(rho(t) approx rho_0 dot ( 1 + a t ))
$
$rho_0$ is resistivity at $t=0$, t is templature.
$alpha$ is called *templature coefficient of resistivity*.


#text(size:10pt)[ たまに千石電商の服きてたらついに指摘されてちょっとうれしい．普通のこの服はデザインセンスがあると思う．高いだけある]

// 物理はすすめずに「区分求積法とは？How does it work?」をやりました．

== Joule heat

$W $ : generating heat by current  - *Joule heat*
$
  bold(W = Q V = I V t)
$

- *Electrical Energy*$[upright(M L T^(-2))]$(電力量) - Work which is done by current(charge)

- *Electrical Power*$[upright(M L T^(-3))]$(電力) - work per unit time

#text(size:10pt)[ 教員：電力量という訳はうれしくない。日本語はscienceに向かない。　　←それ]

$
  bold(P"(power)" = I V = I^2 R)
$

work which done by field $ W = e dot V/l dot v t  dot n S l = e n v S dot V dot t = I V t$


//#text(size:10pt)[ 教員：日本語はscienceに向いていない　←用語だけenglishにするのは割とアリだと思うんだけどな…]

#text(size:10pt)[ ※ Thermal energyは、定義が"ambiguous"なため議論には使えない(使いづらい)]

#text(size:10pt)[ heatは「macroscopicな力学で説明できないenergyの移動(energy transfer)」と定義されている]

#text(size:10pt)[ "heatが発生する"という記述ができる　高温な物体は何かしらの方法(emisionなど)で"heatを発生させる"から、"抵抗のある導体に電流が流れると*熱が発生する*。これをジュール熱と呼ぶ。"という記述ができるっぽい。「熱」って高校物理で直感的じゃない定義ナンバーワンじゃないか…？]

// 授業で初「perfect answer」が出ました
// 

#pagebreak()

== Composite of Resistors

#align(center,box(width:15cm, height:8cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let resistor(l,p,q)={
        let (x1,y1) = p
        let (x2,y2) = q
        let d1 = x2 -x1
        let d2 = y2 -y1
        let d_a = calc.sqrt(d1*d1 + d2*d2)
        let d_e1 = d1/d_a
        let d_e2 = d2/d_a

        let d_a7 = d_a/7

        let rot(a,b)={
          let (a1,a2) = a
          let (b1,b2) = b
          let x_ = b1*d_e1 - b2*d_e2
          let y_ = b1*d_e2 + b2*d_e1
          let a_2 = (a1+x_,a2+y_)
          return a_2
        }


        let pre = (x1,y1)
        let dif = (d_a7,-l)
        let a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
      }

      line((0,4),(0,3))
      resistor(0.5,(0,3),(0,1))
      line((0,1),(0,-1))
      resistor(0.5,(0,-1),(0,-3))
      line((0,-3),(0,-4))

      content((1,2),$R_1$)
      content((1,-2),$R_2$)
      content((0,0), highlight(fill:white)[　])
      content((0,0),$bold(R = R_1 + R_2)$)


      line((9,4),(9,3))
      line((8,3),(10,3))
      line((8,3),(8,1))
      line((10,3),(10,1))
      resistor(0.5,(8,1),(8,-1))
      resistor(0.5,(10,1),(10,-1))
      line((8,-3),(10,-3))
      line((8,-3),(8,-1))
      line((10,-3),(10,-1))
      line((9,-4),(9,-3))

      content((9,0),$R_1$)
      content((11,0),$R_2$)
      content((9,-2), highlight(fill:white)[　　　　　　])
      content((9,-2),$bold(R^(-1) = R_1^(-1) + R_2^(-1))$)


    })
  ]
])
// 抵抗描画便利

== Ammeter and Voltimeter
=== Ammeter

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let resistor(l,p,q)={
        let (x1,y1) = p
        let (x2,y2) = q
        let d1 = x2 -x1
        let d2 = y2 -y1
        let d_a = calc.sqrt(d1*d1 + d2*d2)
        let d_e1 = d1/d_a
        let d_e2 = d2/d_a

        let d_a7 = d_a/7

        let rot(a,b)={
          let (a1,a2) = a
          let (b1,b2) = b
          let x_ = b1*d_e1 - b2*d_e2
          let y_ = b1*d_e2 + b2*d_e1
          let a_2 = (a1+x_,a2+y_)
          return a_2
        }


        let pre = (x1,y1)
        let dif = (d_a7,-l)
        let a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
      }

      line((-0.1,-0.5),(-0.1,-1.5))
      line((0.1,-0.7),(0.1,-1.3))
      line((-0.1,-1),(-2,-1))
      line((0.1,-1),(2,-1))
      line((-2,-1),(-2,1))
      line((2,-1),(2,1))
      line((-2,1),(-1.5,1))
      resistor(0.2,(-1.5,1),(-0.5,1))
      line((-0.5,1),(0.5,1))
      line((1.5,1),(2,1))
      circle((1,1),radius:0.5)

      content((1,1),"A")

      line((5,0),(6.5,0))
      line((5.5,0),(5.5,-1))
      line((5.5,-1),(6.5,-1))
      circle((7,0),radius:0.5)
      resistor(0.2,(6.5,-1),(7.5,-1))
      line((7.5,0),(9,0))
      line((8.5,0),(8.5,-1))
      line((7.5,-1),(8.5,-1))

      content((7,0),"A")
      content((7,-1.5),$R_A$)
      content((7,0.8),$R_O$)
      content((10,2),[indicates $I_O = I / n$])
      line((7.3,0.3),(10,1.8))

      content((10,-2),[$n = 1 + R_O / R_A$])
    })
  ]
])

=== Voltimeter


#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let resistor(l,p,q)={
        let (x1,y1) = p
        let (x2,y2) = q
        let d1 = x2 -x1
        let d2 = y2 -y1
        let d_a = calc.sqrt(d1*d1 + d2*d2)
        let d_e1 = d1/d_a
        let d_e2 = d2/d_a

        let d_a7 = d_a/7

        let rot(a,b)={
          let (a1,a2) = a
          let (b1,b2) = b
          let x_ = b1*d_e1 - b2*d_e2
          let y_ = b1*d_e2 + b2*d_e1
          let a_2 = (a1+x_,a2+y_)
          return a_2
        }


        let pre = (x1,y1)
        let dif = (d_a7,-l)
        let a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
      }

      line((-0.1,-0.5),(-0.1,-1.5))
      line((0.1,-0.7),(0.1,-1.3))
      line((-0.1,-1),(-2,-1))
      line((0.1,-1),(2,-1))
      line((-2,-1),(-2,1))
      line((2,-1),(2,1))

      line((-2,1),(-0.5,1))
      resistor(0.2,(-0.5,1),(0.5,1))
      line((0.5,1),(2,1))

      circle((0,2),radius:0.5)

      content((0,2),"V")


      line((-1,1),(-1,2))
      line((-1,2),(-0.5,2))
      line((1,2),(0.5,2))
      line((1,1),(1,2))


      line((5,0),(7.5,0))

      circle((6,0),radius:0.5, fill:white)
      content((6,0),"V")
      content((6,-0.9),$R_O$)

      resistor(0.2,(7.5,0),(8.5,0))
      content((8,-0.7),$R_A$)

      line((8.5,0),(9,0))

      line((6.3,0.3),(10,1.8))
      content((10,2),[indicates $V_O = V / n$])

      content((10,-2),[$n = 1 + R_A / R_O$])
    })
  ]
])

#pagebreak()

=== How to Determine Electromotive Force

- *Electromotive Force* - "起電力"


#align(center,box(width:15cm, height:5.5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let resistor(l,p,q)={
        let (x1,y1) = p
        let (x2,y2) = q
        let d1 = x2 -x1
        let d2 = y2 -y1
        let d_a = calc.sqrt(d1*d1 + d2*d2)
        let d_e1 = d1/d_a
        let d_e2 = d2/d_a

        let d_a7 = d_a/7

        let rot(a,b)={
          let (a1,a2) = a
          let (b1,b2) = b
          let x_ = b1*d_e1 - b2*d_e2
          let y_ = b1*d_e2 + b2*d_e1
          let a_2 = (a1+x_,a2+y_)
          return a_2
        }


        let pre = (x1,y1)
        let dif = (d_a7,-l)
        let a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
      }



      line((-0.1,-0.5),(-0.1,-1.5))
      line((0.1,-0.7),(0.1,-1.3))
      content((0,-1.8),$bold(E), bold(r)$)

      line((-0.1,-1),(-2,-1))
      line((0.1,-1),(2,-1))

      line((-2,-1),(-2,2))
      line((2,-1),(2,2))

      line((-2,1),(-0.5,1))
      resistor(0.2,(-0.5,1),(0.5,1))
      line((-0.5,0.7),(0.5,1.3),mark:(end:">", fill:black))
      content((0,0.5),$bold(R)$)
      line((0.5,1),(2,1))

      circle((0,2),radius:0.5)
      content((0,2),"V")

      line((-2,2),(-0.5,2))
      line((2,2),(0.5,2))

      circle((-2,0),radius:0.5, fill:white)
      content((-2,0),"A")

      line((-2.2,0.7),(-2.2,2), mark:(end:">", fill:black))
      content((-2.5,1.3),$bold(I)$)

      line((-2,2.6),(2,2.6), mark:(start: ">",end:">", fill:black))
      line((-2,3),(-2,2))
      line((2,3),(2,2))
      content((0,3),$bold(V)$)
    })
  ]
])
$V = E - r I
$

Observing multiple $(V,I)$ points, we can determine $E$ and $I$.

=== How to Determine Resistance
*Wheatstone bridge*


#align(center,box(width:15cm, height:6.5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      let resistor(l,p,q)={
        let (x1,y1) = p
        let (x2,y2) = q
        let d1 = x2 -x1
        let d2 = y2 -y1
        let d_a = calc.sqrt(d1*d1 + d2*d2)
        let d_e1 = d1/d_a
        let d_e2 = d2/d_a

        let d_a7 = d_a/7

        let rot(a,b)={
          let (a1,a2) = a
          let (b1,b2) = b
          let x_ = b1*d_e1 - b2*d_e2
          let y_ = b1*d_e2 + b2*d_e1
          let a_2 = (a1+x_,a2+y_)
          return a_2
        }


        let pre = (x1,y1)
        let dif = (d_a7,-l)
        let a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,2*l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
          dif = (d_a7,-l)
        a = rot(pre,dif)
        line(pre,a)
        pre = a
      }


      line((-0.1,-0.5),(-0.1,-1.5))
      line((0.1,-0.7),(0.1,-1.3))
      content((0,-1.8),$bold(V)$)

      line((-0.1,-1),(-4,-1))
      line((0.1,-1),(4,-1))

      line((-4,-1),(-4,2))
      line((4,-1),(4,2))

      line((-4,2),(-3,2))
      line((4,2),(3,2))

      line((-3,2),(-2,2 + 1 * 2 / 3))
      resistor(0.2,(-2,2 + 1 * 2 / 3),(-1,2 + 2 * 2 / 3))
      line((-1,2 + 2 * 2 / 3),(0,2 + 3 * 2 / 3))
      line((3,2),(2,2 + 1 * 2 / 3))
      resistor(0.2,(2,2 + 1 * 2 / 3),(1,2 + 2 * 2 / 3))
      line((0.9,3),(2.1,3),mark:(end:">", fill:black))
      line((1,2 + 2 * 2 / 3),(0,2 + 3 * 2 / 3))

      line((-3,2),(-2,2 - 1 * 2 / 3))
      resistor(0.2,(-2,2 - 1 * 2 / 3),(-1,2 - 2 * 2 / 3))
      line((-1,2 - 2 * 2 / 3),(0,2 - 3 * 2 / 3))
      line((3,2),(2,2 - 1 * 2 / 3))
      resistor(0.2,(2,2 - 1 * 2 / 3),(1,2 - 2 * 2 / 3))
      line((1,2 - 2 * 2 / 3),(0,2 - 3 * 2 / 3))

      line((0,0),(0,4))

      circle((0,2),radius:0.5, fill:white)
      content((0,2),"G")

      content((-1.5,3.8),$R_1$)
      content((-1.5,0.2),$R_2$)
      content((1.5,3.8),$R_3$)
      content((1.5,0.2),$R_x$)

    })
  ]
])
When current at G equals to $0$,

$R_1 I_1 = R_2 I_2\
R_3 I_1 = R_x I_2\
<=> 
$

== Charge of Capacitor

When a capacitor is connetced with a battery, charge of capacitor is proportional to $exp(-t/(R C))$ ( $R$ is resistance of circuit, $C$ is capacity )

#pagebreak()

= Semiconductor

- *Intrinsic Semiconductor*真性半導体 - Simple substance of *$"Si"$* or *$"Ge"$*. In low templature, they cannot conduct electron. In high templature, They can conduct electron.
- *Extrinsic Semiconductor*不純物半導体 - Substance which have trace amount of *$"P"$* or *$"Al"$* in *$"Si"$* or *$"Ge"$*.

\


- *n-type semiconductor* ( negative )
*$"P"$* have $4+1$ electrons on outermost shell.\
$=>$ They have extra electrons.

- *p-type semiconductor* ( positive )
*$"Al"$* have $4-1$ electrons on outermost shell.\
$=>$ They have hole of electrons ( positive hole ).
#text(size:10pt)[( なんでこれ共有結合できるの？の？？オクテッドソクみたさなひ)]


== P-N Junction / Diode
#align(center,box(width:15cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-2,1),(0,-1))
      rect((2,1),(0,-1))
      line((-2,0),(-2.5,0))
      line((2,0),(2.5,0))
      line((-2.5,0),(-2.5,-2))
      line((2.5,0),(2.5,-2))
      line((-2.5,-2),(-0.1,-2))
      line((2.5,-2),  (0.1,-2))

      line((-0.1,-2 + 0.5),(-0.1,-2 - 0.5))
      line((0.1,-2 + 0.3),(0.1,-2 - 0.2))

      content((1,1.5),[N])
      content((-1,1.5),[P])

      let elec(p,q)={
        let (x,y) = p
        let (x_,y_) = q
        circle((x,y),radius:0.12, fill:white)
        content((x,y+0.05),"-")
        line((x,y),(x+x_,y+y_),mark:(end:">", fill:black))
      }
      let hole(p,q)={
        let (x,y) = p
        let (x_,y_) = q
        circle((x,y),radius:0.12, fill:white)
        content((x,y),"+")
        line((x,y),(x+x_,y+y_),mark:(end:">", fill:black))
      }
      elec((2,0),(-0.5,0))
      elec((1,0),(-0.5,0))
      elec((1.5,0.5),(-0.5,0))
      elec((1.5,-0.5),(-0.5,0))
      elec((0.5,0.5),(-0.5,0))
      elec((0.5,-0.5),(-0.5,0))
      elec((0.1,0),(0,0))
      hole((-0.1,0),(0,0))

      elec((-2,0),(-0.5,0))
      hole((-1.8,0),(0.5,0))
      hole((-1,0),(0.5,0))
      hole((-1.5,0.5), (0.5,0))
      hole((-1.5,-0.5),(0.5,0))
      hole((-0.5,0.5), (0.5,0))
      hole((-0.5,-0.5),(0.5,0))

      circle((0,0),radius:0.4,stroke:(paint:red))
      line((0.25,0.25),(3,2.5),stroke:(paint:red))
      content((3,2.5),text(fill:red)[#highlight(fill:white)[recombination]])



    })
  ]
])

#pagebreak()

== Transistor

=== P-N-P Transistor
#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-2,1),(0.2,-1))
      rect((2.5,1),(-0.2,-1))

      line((-2,0),(-3,0))
      line((2.5,0),(3,0))
      line((0,-1),(0,-2))
      content((-3.2,0.5),[Emitter])
      content((3.5,0.5),[Collector])
      content((1,-2),[Base])

      content((-1,0),[P])
      content((1,0),[P])
      content((0,0),[N])

      line((6,0),(8,0),stroke:(thickness:0.1))
      line((5.5,1.5),(6.5,0),mark:(end:">",fill:black),stroke:(thickness:0.08))
      line((7.5,0),(8.5,1.5),stroke:(thickness:0.08))
      line((7,0),(7,-1.5),stroke:(thickness:0.08))
      content((5.3,1),[E])
      content((8.7,1),[C])
      content((7.3,-1),[B])

    })
  ]
])
#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-2,1),(0.2,-1))
      rect((2.5,1),(-0.2,-1))

      line((-2,0),(-3,0))
      line((2.5,0),(3,0))
      line((0,-1),(0,-2))
      content((-3.2,0.5),[Emitter])
      content((3.5,0.5),[Collector])
      content((1,-2),[Base])

      content((-1,0),[N])
      content((1,0),[N])
      content((0,0),[P])

      line((6,0),(8,0),stroke:(thickness:0.1))
      line((5.5,1.5),(6.5,0),mark:(start:">",fill:black),stroke:(thickness:0.08))
      line((7.5,0),(8.5,1.5),stroke:(thickness:0.08))
      line((7,0),(7,-1.5),stroke:(thickness:0.08))
      content((5.3,1),[E])
      content((8.7,1),[C])
      content((7.3,-1),[B])

    })
  ]
])


- Base semiconductor is very thin.

#text(size:10pt)[( なんでこれスイッチングできる？？It's too dificult to understand.)]


#text(size:10pt)[　(P-N Junctionすると，recombination（再結合）がある程度起きて平衡になる．そのときN→Pへの電場が
発生していて，そのおかげでEmitterからCollectorには電子が移動できない．

その電場をEmitter-Base間の電圧で相殺することで，ポテンシャルの壁が消えて流れるようになる．

ってこと？Emitte-Base間に電流が流れてしまうのは副産物的なもの？ // id 4e7db33c-3176-4dc2-bf01-34091a9b9a91

なおBase層が薄いのは，potentialだけじゃ説明できなくて，electron/holeが失われる量を

防ぐため，と理解(あってるのだろうか…))]

#text(size:10pt)[　(ついでに電池というものを「酸化還元の動的平衡」と捉えるべきであると思った // なお、これは id 477f8546-af77-46f3-b670-c2adf4b64384 で解決済み。
電流が流れ続けるのをLeChatelierの原理で説明できるし，電圧が発生するのも非常に直感的．教科書を書き換えよう))]



//amplifyは、増幅という意味。音楽バンドなどが使うアンプという言葉でよく知られている。

//1学期期末考査の範囲はここまで。

#pagebreak() // ここから別単元。2学期中間考査はここから。

= Electric Current and Magnetic Field

== Magnetic Field

- *magnetic pole* - *N*(north pole) and *S*(south pole) exist in pairs. This pair called magntic pole.
- *magnetic charge* [$"Wb"$] - the strength of ability to generate magnetic field. *north pole have positive value* and south pole have negative value.


#align(center,box(width:15cm, height:4cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-3,5),(-2,0),fill:rgb("dddddd"))
      rect((3,-5),(2,0),fill:rgb("dddddd"))


      circle((-2.5,0),radius:0.07,fill:black)
      line((-2.5,0),(-0.5,0), mark:(end:">", fill:black),stroke:(thickness:0.05))
      circle(( 2.5,0),radius:0.07,fill:black)
      line((2.5,0),(0.5,0), mark:(end:">", fill:black),stroke:(thickness:0.05))

      content((-2.5,0.5),text(fill:red)[N])
      content((2.5,-0.5),text(fill:blue)[S])

      content((-1.5,-0.3),$bold(F)$)
      content((1.5,-0.3),$bold(F)$)

      line((-2.5,-1.5),(2.5,-1.5), mark:(start:">",end:">", fill:black))
      line((-2.5,0.2),(-2.5,-1.6))
      line((2.5,0.2),(2.5,-1.6))

      content((0,-1.7),$bold(r)$)


      content((-3.5,0.0),$bold(m_1)$)
      content((3.5,0.0),$bold(m_2)$)

    })
  ]
])



$
  bold(F = k_m dot (m_1 m_2) / r^2)
$

$m_1$,$m_2$ is *magnetic charge*.

=== Magnetization
- *magnetization* 磁化 - the phenomen of being polarized into N ans S by magnetic field.

#v(0.5em)

- *ferromagnet* 強磁性体 - substances which are magnetized strongly.
- *paramagnet* 常磁性体- substances which are magnetized slightly.
- *diamagnet* 弱磁性体- substances which are magnetized in the reverse direction.


=== Magnetic Field generated by Electric Current
#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      ortho(x:30deg, y:150deg,{
        let AXIS_LENGTH = 2.5

        line((0,0,-3),(0,0,3),mark:(end:">",fill:red),stroke:(paint:red,thickness:0.1))
        content((0,-0.7,2),text(fill:red)[$bold(I)$])

        line((0,0,-3),(0,0,-0.05),stroke:(paint:red,thickness:0.1))

        drawc((0,0,0),0.1,(0,180deg,0),blue,0.04)
        drawc((0,0,0),0.2,(0,180deg,0),blue,0.04)
        drawc((0,0,0),0.4,(0,180deg,0),blue,0.04)
        drawc((0,0,0),0.8,(0,180deg,0),blue,0.04)
        drawc((0,0,0),1.6,(0,180deg,0),blue,0.04)
        drawc((0,0,0),3.6,(0,180deg,0),blue,0.04)

        line((0,0,0),(-2,1,0),mark:(start:">",end:">",fill:black))
        content((-1.0,1.0,0),$bold(r)$)
        content((-2,1,0),text(fill:blue)[・])
        line((-2,1,0),(-2 - 1, 1 - 2,0),mark:(end:">",fill:blue),stroke:(paint:blue,thickness:0.1))
        content((-2.1,0,0),text(fill:blue)[$bold(H)$])

      })
    })
  ]
])

Let $bold(H)$ magnetic field, $bold(r)$ distance from wire.
$
  bold(H = I / (2 pi r))
$

$"[N/Wb]"$ = $"[A/m]"$

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      ortho(x:30deg, y:150deg,{
        let AXIS_LENGTH = 2.5
        //line((0, 0, 0), (AXIS_LENGTH, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
        //line((0, 0, 0), (0, AXIS_LENGTH, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
        //line((0, 0, 0), (0, 0, AXIS_LENGTH), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
        //content("x-axis.end", [$x$], anchor: "west")
        //content("y-axis.end", [$y$], anchor: "south")
        //content("z-axis.end", [$z$], anchor: "north-east")

        drawc((0,0,0),2,(0,180deg,0),red,0.1)
        let angl = 60deg
        drawc((0,2,0),0.2,(angl,90deg,0),blue,0.04)
        drawc((0,2,0),0.4,(angl,90deg,0),blue,0.04)
        drawc((0,2,0),1.0,(angl,90deg,0),blue,0.04)
        for i in range(3){
          angl += 90deg
          drawc((0,2,0),0.2,(angl,90deg,0),blue,0.04)
          drawc((0,2,0),0.4,(angl,90deg,0),blue,0.04)
          drawc((0,2,0),1.0,(angl,90deg,0),blue,0.04)
        }

        line((0,0,-3),(0,0,3),mark:(end:">",fill:blue),stroke:(paint:blue,thickness:0.1))
        content((0,2.5,0),text(fill:red)[$bold(I)$])

        line((-2,0,0),(0,0,0),mark:(start:">",end:">",fill:black))
        content((-1,0.3,0),$bold(r)$)
        content((-0.2,0.2,1),text(fill:blue)[$bold(H)$])

      })
    })
  ]
])

$
  bold(H = I / (2 r))
$
#text(size:10pt)[( さすがにこの相互作用の説明はされないのか…)]

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      ortho(x:30deg, y:150deg,{
        let AXIS_LENGTH = 2.5
        //line((0, 0, 0), (AXIS_LENGTH, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
        //line((0, 0, 0), (0, AXIS_LENGTH, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
        //line((0, 0, 0), (0, 0, AXIS_LENGTH), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
        //content("x-axis.end", [$x$], anchor: "west")
        //content("y-axis.end", [$y$], anchor: "south")
        //content("z-axis.end", [$z$], anchor: "north-east")

        drawc((0,0,0),2,(0,180deg,0),red,0.1)
        let angl = 60deg
        drawc((0,2,0),0.2,(angl,90deg,0),blue,0.04)
        drawc((0,2,0),0.4,(angl,90deg,0),blue,0.04)
        drawc((0,2,0),1.0,(angl,90deg,0),blue,0.04)
        for i in range(3){
          angl += 90deg
          drawc((0,2,0),0.2,(angl,90deg,0),blue,0.04)
          drawc((0,2,0),0.4,(angl,90deg,0),blue,0.04)
          drawc((0,2,0),1.0,(angl,90deg,0),blue,0.04)
        }

        line((0,0,-3),(0,0,3),mark:(end:">",fill:blue),stroke:(paint:blue,thickness:0.1))
        content((0,2.5,0),text(fill:red)[$bold(I)$])

        line((-2,0,0),(0,0,0),mark:(start:">",end:">",fill:black))
        content((-1,0.3,0),$bold(r)$)
        content((-0.2,0.2,1),text(fill:blue)[$bold(H)$])

      })
    })
  ]
])

=== Force

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      ortho(x:-70deg, y:-00deg,z:-20deg,{
        //let AXIS_LENGTH = 2.5
        //line((0, 0, 0), (AXIS_LENGTH, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
        //line((0, 0, 0), (0, AXIS_LENGTH, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
        //line((0, 0, 0), (0, 0, AXIS_LENGTH), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
        //content("x-axis.end", [$x$], anchor: "west")
        //content("y-axis.end", [$y$], anchor: "south")
        //content("z-axis.end", [$z$], anchor: "north-east")

        line((0,0,0),(2,0.5,0),mark:(end:">",fill:red),stroke:(paint:red))
        line((0,0,0),(0,2,0),mark:(end:">",fill:blue),stroke:(paint:blue))
        line((0,0,0),(0,0,3),mark:(end:">",fill:black))

      })
    })
  ]
])
$
  bb(F) = bb(I) times bb(B) dot l
$

//類似性＝アナロジー // 2026-06-29の名言
//物理においては、同じ式は同じような扱い方ができる。 // id ce63c80b-db29-4d48-a21a-d900ac79b7ed


// typstのcetzで，3次元で，circleを描くときに，面を傾けるのはどうすればいいのでしょう

// 陽イオンの振動→高い温度→熱が高いところ(導線)から低いところ(空気)へ伝わる→うわあっつ!! -> id 7b191f20-ba90-454f-9cb7-78cf2c32d381

// IDE「電位差があることと電場が生まれることの間にはどのような関係があるか？」 -> id 45e72069-e833-4182-b569-9a4d41c3eb48
// how is this:電位は単位電荷の持つpotential energyなので，電位が位置によって違うということは，そこには力の場が存在する必要があるから



// spacing in equation  :  thin med thick quad wide
// 保存力 -> id 0271b48f-0d66-47be-921e-f6bb8fb2c447
//#text(size:10pt)[( なんでこれ共有結合できるの？の？？オクテッドソクみたさなひ)]

//高校物理「仕事は保存される」 
//大学の古典力学「$integral (dt K(t)-U(t))$が停留点となる経路が実現される」


/*
#align(center,box(width:15cm, height:6cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *
    })
  ]
])
*/
//#text(size:10pt)[( 電気力線ってsuperpositionできるの？uniformであることは積分で証明？)]

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

/*

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      ortho(x:30deg, y:150deg,{
        let AXIS_LENGTH = 2.5
        //line((0, 0, 0), (AXIS_LENGTH, 0, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "x-axis")
        //line((0, 0, 0), (0, AXIS_LENGTH, 0), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "y-axis")
        //line((0, 0, 0), (0, 0, AXIS_LENGTH), mark: (end: ">", fill:black),stroke:(thickness:0.02), name: "z-axis")
        //content("x-axis.end", [$x$], anchor: "west")
        //content("y-axis.end", [$y$], anchor: "south")
        //content("z-axis.end", [$z$], anchor: "north-east")
      })
    })
  ]
])
*/
