# 中级

你已经能画出一只静态的、剪影清楚的精灵。这一级练手艺：色阶和色相偏移、描边、小尺寸可读性、第一种非正面机位、待机和四帧级的走路，以及布、植物、风、昆虫、金属这些还能用很少的帧讲清的运动。

专业的八帧人体、四足步序、翅膀的重力，以及游客视点和航拍的成对作业，放到 [高级](advanced.md)。打开条目前先打开 URL。

## 走路

### Pixelblog - 8 - Intro to Animation

- 作者：Raymond Schlitter（SLYNYRD）
- 链接：https://www.slynyrd.com/blog/2018/8/19/pixelblog-8-intro-to-animation
- 主题：待机、跑步关键帧、帧数和力气
- 为什么要看：在做完整人走之前，先学会循环里的每一帧都会被反复看见，簇的错误藏不住。
- 打开后抓住：待机可以只是呼吸或极小的弹动，一个游戏里的待机要统一。跑步先摆手脚打开的大姿势和四肢收回的通过帧，再决定要不要中间帧。帧加多会更滑，不一定更有力。播放速度和帧数一起改。

### Walk Cycles, Part 1

- 作者：Jason Perry（finalbossblues）
- 链接：http://finalbossblues.com/walk-cycles-p1/
- 主题：侧面走、着地脚后滑、四帧与八帧
- 为什么要看：把“身体留在原地、着地脚向后推”讲成游戏精灵的基本错觉，并说明侧面最能看见肢体。
- 打开后抓住：经典的踏步—站立—踏步会把站立帧塞进循环，腿像在搓地。至少用四个各司其职的姿势，能看清同一条腿向前、落下、向后。八帧是在这四个关键姿势之间加密。正面和背面更容易取巧，所以教学放在侧面。

### Walk Cycles, Part 2

- 作者：Jason Perry（finalbossblues）
- 链接：http://finalbossblues.com/walk-cycles-part-2/
- 主题：正面走、小幅度、不要用站立帧充数
- 为什么要看：正面看不见完整的前后交叉。这篇演示小幅的抬—落—推，以及站立帧如何把走路变成拖步。
- 打开后抓住：头和躯干可以复用，力气放在四肢。迈步帧可以比站立矮一个像素。后面的肢体收掉高光。角色左右不对称，就不能整帧镜像交差。这一级的正面步幅保持很小。

## 视角

### The RPG Base’s Four Directions

- 作者：Jason Perry（finalbossblues）
- 链接：https://finalbossblues.com/the-rpg-bases-four-directions/
- 主题：正面、背面、侧面的同一身体
- 为什么要看：第一次把同一个人转个方向时，最常见的事故是身高和肩宽对不上。
- 打开后抓住：用同一组水平参考线卡住头顶、肩、胯和脚。背面可以借用正面的剪影再改。侧面要重画宽度：太薄，转身会突然变纸片；太厚，转身又会肿。重要角色不要靠镜像敷衍侧面。

### Pixelblog - 3 - Graphical Projection Part 1

- 作者：Raymond Schlitter（SLYNYRD）
- 链接：https://www.slynyrd.com/blog/2018/3/14/pixelblog-3-graphical-projections-1
- 主题：四分之三俯视、四分之三侧面
- 为什么要看：用同一栋房子比较两种最常用的斜视角，并写明全场只能有一套规则。
- 打开后抓住：四分之三俯视像从斜上方看，顶和正面都在。四分之三侧面像平视墙角。作者宁可保持干净的斜线，也不强求真实透视里的每一点收缩。深度主要来自转角上的明暗，不来自多加消失点。

### Pixelblog - 4 - Graphical Projection Part 2

- 作者：Raymond Schlitter（SLYNYRD）
- 链接：https://www.slynyrd.com/blog/2018/4/12/pixelblog-4-graphical-projection-part-2
- 主题：等距、2:1 台阶线
- 为什么要看：等距是像素里最常见的“能看见顶和两面墙”的画法，这篇给出能在网格上重复的线。
- 打开后抓住：用 2:1 的台阶画斜边，并在全图复用。先定色板、光源和比例。同一条斜边可以复制，不必每一笔重数。等距好看，也比正侧面费工；控制类游戏会更难读方向。

### Chapter 2: Basic Shapes

- 作者：pixelbath
- 链接：https://pixelbath.com/isometric-pixel-art-guide/chapter-2-basic-shapes/
- 主题：等距直线、网格、立方体
- 为什么要看：在画房子之前，先把一条干净的等距线和一个立方体搭对。
- 打开后抓住：斜线保持统一的 2:1 或 1:1。30° 的“真等距”在像素上不直。平行线没有消失点，远处的箱子不会变小。先在平面上想好长宽，再折进等距网格。

### Chapter 3: Perspectives

- 作者：Antoine Gersant（Les Forges）
- 链接：https://opengameart.org/content/chapter-3-perspectives
- 主题：等距、平面斜投影、四分之三
- 为什么要看：用短文把几种轴测投影并排，提醒四分之三里所有水平面要同一套缩短。原文是英译，句子不顺，以图和术语为准。
- 打开后抓住：等距用 1:2 线。另一种斜投影用 1:1。四分之三是很多 RPG 的默认机位。选定之后，地面的格子和墙的顶边用同一规则，不要这张砖是等距、下一张砖又是正顶。

## 运动轨迹

### Fabric / Flags

- 作者：Pedro Medeiros（saint11）
- 链接：https://github.com/saint11/Saint11Tutorials/blob/master/Fabric.gif
- 主题：布、旗帜、跟随
- 为什么要看：用一张图把布的波形、褶皱方向和“布要服从身体”放在一起。
- 打开后看图，抓住：布在风里走波形，波峰沿风向错开。先画一根引导线。褶皱从被压住或被拽住的点散开，紧的地方顺着拉力，松的地方更横。暗的地方少画褶。大多数布只有软阴影，没有硬高光。衣服在动，骨头的长度不变。

### Pixelblog - 33 - Wind Effects

- 作者：Raymond Schlitter（SLYNYRD）
- 链接：https://www.slynyrd.com/blog/2021/7/16/pixelblog-33-wind-effects
- 主题：风、头发、旗帜、树和落叶
- 为什么要看：风本身没有外形。这篇教你用布、灰尘和植物把风画出来，而且不把构图打散。
- 打开后抓住：沿着会被吹到的边缘设流动点，波从根部小、到末端大，间距相等才能循环。树冠分成几层，每层做很小的上、侧、下位移，错开之后就像一阵风吹过去。落叶按固定像素步进移动，再用缺角暗示翻面。

### Vegetation tutorial - part 1

- 作者：Pedro Medeiros（saint11）
- 链接：https://github.com/saint11/Saint11Tutorials/blob/master/Vegetation.gif
- 主题：植物的形、光和簇
- 为什么要看：树和灌木是场景里最容易画成噪声的材料。这篇要求剪影先能认出来。
- 打开后看图，抓住：先剪影，再一块体积光，再重复的叶子簇。色要少。叶子用同一套小图案。暖色的亮部和冷色的阴影分开。悬垂和重复可以很吵，所以内部留出眼睛能休息的平涂。

### Ant Walk Cycles

- 作者：Ian Parberry
- 链接：https://ianparberry.wordpress.com/2013/02/01/ant-walk-cycles/
- 主题：昆虫的三脚架步态
- 为什么要看：六条腿不是缩小的人腿。这篇用蚂蚁说明怎样始终有三只脚着地。
- 打开后抓住：左前、左后和右中是一组，另外三条是另一组。一组向后撑地时，另一组抬起向前。四帧可以是：中立、一组向前、中立、另一组向前。不要给昆虫画膝关节朝后的人类小腿。

### Metals

- 作者：Pedro Medeiros（saint11）
- 链接：https://github.com/saint11/Saint11Tutorials/blob/master/Metal.gif
- 主题：金属的光和刚性
- 为什么要看：金属的运动轨迹和布相反。它不拉伸，只绕自己的零件转。
- 打开后看图，抓住：先固有色，再一块主光、一块暗部、一条背光。亮部更冷、更少饱和，暗部可以更饱和。高光小。划痕说明这件东西被用过。反射可以简化成几条明暗带。零件分开画，角度变了就改高光的位置。

## 色与线

这些手艺会被三条柱同时用到。小步幅的正面走、等距斜线和树冠，都依赖干净的线和分得开的色。

### 6 - Basic Color Theory

- 作者：Pedro Medeiros（saint11）
- 链接：https://saint11.art/pixel_art_articles/article6/
- 主题：色相、饱和、明度、色相偏移
- 为什么要看：只拉明暗的色阶发闷。这篇说明光和阴影还可以在冷暖上分开。
- 打开后抓住：大面积用较低的饱和，小面积用高饱和当口音。不确定时让亮部更暖、暗部更冷，或按场景反过来，但要全图一致。一种色相走遍明暗是可用的风格；互补色很容易吵。

### Pixelblog - 1 - Color Palettes

- 作者：Raymond Schlitter（SLYNYRD）
- 链接：https://www.slynyrd.com/blog/2018/1/10/pixelblog-1-color-palettes
- 主题：色阶、色相偏移、从大色板里抽小色板
- 为什么要看：把“一条坡上的色要能互相搭配”做成可以重复的调色方法。
- 打开后抓住：用色相、饱和、明度来调，不要只加白加黑。明度往亮走时，饱和通常要降下来，否则高光刺眼。色相沿着明度轻轻偏转，不同色条之间才好衔接。一张图只用其中很少的几格。最后仍要靠眼睛改，公式只是起点。

### Pixel Art Outlines

- 作者：Sam Keddy（Skeddles）
- 链接：https://lospec.com/articles/pixel-art-outlines/
- 主题：单像素描边、斜线、曲线、哪里不描
- 为什么要看：描边是分离形体的工具。这篇讲清楚多细、多直，以及脚底和握手处为什么要断开。
- 打开后抓住：单像素轮廓的斜段只在角上相接，粗细才均匀。直线由等长的小段重复。圆在上下左右的线段最长，靠近斜向逐渐变短。外轮廓可以保留，内部能靠颜色分开的地方不必再圈一圈黑线。挨着地面和握住的物体处，描边断开，对象才像真的碰到了。

### Outlines

- 作者：Pedro Medeiros（saint11）
- 链接：https://github.com/saint11/Saint11Tutorials/blob/master/Outlines.gif
- 主题：选择性描边、内外轮廓
- 为什么要看：在 Keddy 的线规之外，补一张关于“描边很占像素，所以要省”的图。
- 打开后看图，抓住：外轮廓把对象从背景上揭下来，内轮廓只在两块形靠色分不开时使用，而且可以更亮。亮的区域甚至可以拿掉描边。描边也能用来强调尖角。低分辨率里，偶尔留下的棱角是细节，不是必须修平的错误。

### Shading

- 作者：Pedro Medeiros（saint11）
- 链接：https://github.com/saint11/Saint11Tutorials/blob/master/Shading.gif
- 主题：色相偏移、平面、明暗交界
- 为什么要看：Lospec 上最常被点开的明暗图。它把“不要只改亮度”画成可以直接对照的坡。
- 打开后看图，抓住：暗部和亮部可以向色轮上不同的方向偏。把色带压短，台阶才看得见。平的面尽量一色，圆的面才出现高光、交界和反弹光。投影的边硬，自身阴影的边可以软一点。逆光和抖动是另一些合法答案，但一张图里选定一种。
