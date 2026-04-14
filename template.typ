/*

#set page(paper: "a4", margin: 2.5cm)
#set text(font: ("Noto Serif CJK JP"), size: 10pt)
#set heading(numbering: "1.")

//#let linspace(start, end, n)=range(0,n).map(x => x / n * (end-start) + start)



#cetz.canvas(length: 3cm, {
  import cetz.draw: *
    // 座標軸
    line((-1.5, 0), (1.5, 0), stroke: (paint: black, thickness: 1pt), name: "x-axis")
    line((0, -1.5), (0, 1.5), stroke: (paint: black, thickness: 1pt), name: "y-axis")
    
    // 軸ラベル
    content((1.5, -0.15), anchor: "north", text(size: 14pt, $x$))
    content((-0.15, 1.5), anchor: "east", text(size: 14pt, $y$))
    content((-0.15, -0.15), anchor: "north-east", text(size: 14pt, $O$))
})

$
dv(f,x),quad dv(f,x,z),quad
$

$ f(x) = x^(2-x)$
*/

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
  font: ("New Computer Modern","Noto Sans CJK JP")
)

//#show regex("[\p{scx:Han}\p{scx:Hira}\p{scx:Kana}]"): set text(font: "BIZ UDPGothic")
//#set text(lang: "ja")
#show figure.caption: set text(font: ("New Computer Modern"),weight: "bold", size: 7pt)

#set enum(numbering: "(1)",)











Let $f(x)=a$.
- 赤い円弧の左側の端点の座標は
  $ k = (1 + sqrt(7))/4 - (1 - sqrt(7))/4 = sqrt(7)/2 $

$ -1 lt.eq k lt.eq sqrt(7)/2 $
$ therefore -1 lt.eq sin theta - cos theta lt.eq sqrt(7)/2 $




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
