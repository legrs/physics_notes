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
#set page(numbering: "1")
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
  #text(size: 15pt)[form 2026-04-14]
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
$V_("AB")$ is also called *voltage*.


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
( if test charge move with a direction perpendicular to electric force , work is $W = bb(F dot Delta x)$ $<=>$ potential is same constantly. )
$
  1/2 m v^2 + q V = "const."
$

#pagebreak()

== Substance and Electric Field

#align(center,box(width:15cm, height:5cm, clip:true)[
  #place(center + horizon)[
    #cetz.canvas({
      import cetz.draw: *

      rect((-1,1.8),(1,-1.8))
      line((-2,2),(2,2), mark:(end:">", fill:black))
      line((-2,1),(2,1), mark:(end:">", fill:black))
      line((-2,0),(2,0), mark:(end:">", fill:black))
      line((-2,-1),(2,-1), mark:(end:">", fill:black))
      line((-2,-2),(2,-2), mark:(end:">", fill:black))
      content((-0.7,1.3), $-$)
      content((-0.7,0.3), $-$)
      content((-0.7,-0.7), $-$)
      content((0.7,1.3), $+$)
      content((0.7,0.3), $+$)
      content((0.7,-0.7), $+$)
      line((-0.5,1.3),(0.5,1.3), mark:(end:">", fill:black))
      line((-0.5,0.5),(0.5,0.5), mark:(end:">", fill:black))
      line((-0.5,-0.7),(0.5,-0.7), mark:(end:">", fill:black))
    })
  ]
])

In conductor, electric field is $bb(0)$.

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
