# 动作、接触与跟随 Action motion

用于攻击、受击等短动作的制作和评审。先确定要传达的力量与节奏，再复用像素、补中间帧；不规定统一步幅、帧数或所有动作都要跨步。

## 关键姿势与发力

- 先摆出动作需要的准备、发力、命中或伸展、收势姿势，按玩法调整时长。普攻通常需要清楚且短的准备、快速发力和可读的恢复，不能只把武器匀速来回移动。
- 看下肢支撑、骨盆、躯干、肩臂和武器之间的发力关系。跨步刺击可以从下沉、后脚蹬地到前脚落定；站桩攻击也可以用屈膝、重心转移和髋肩转动表现力量。坐姿、悬浮、机械或刻意僵硬的角色按自身结构设计，不硬加迈步。
- 保持骨长、握持点和轮廓体积。检查重心是否有支撑、落地是否承重、收势是否回稳。布和火焰不能遮住身体没有参与动作的问题。

*Block the action before optimizing edits. Judge the support-to-weapon chain against the intended move. A planted attack can use knee compression and hip/shoulder motion; stepping is not mandatory. Keep limb lengths, grips, and volume coherent.*

## 接触与坐标系

先区分支撑、离地摆动、再接触，并说明精灵是原地播放、带根位移，还是由游戏移动角色。

- 支撑脚在接触阶段相对地面稳定；躯干可以前移或下沉。抬脚、跨步、转身和蹬地时，脚的位置和姿态可以改变。不要把整个动作的两只脚都钉死。
- 原地走路循环里，支撑脚可相对画布向后移动，以配合游戏中的角色前进。带根位移时，结合根位移检查脚相对世界的轨迹。仅有像素差异或脚底横移，不能直接判为脚滑。
- 用脚底或接触锚点判断接触，不用飘动红绫、影子、火焰或轮子效果的最低像素代替脚底。无法取得根位移或接触设计时，评审写明尚未核实的条件。

*Judge planted contacts relative to the ground, not every changed pixel. In-place locomotion can move the support foot backward on the canvas. Airborne feet may move. Effects below the feet do not define contact.*

## 布料与附属物

由身体上的挂接点带动红绫、头发、枪缨等自由端。挂接点跟住身体；自由端随加速滞后、减速后继续前摆，再回摆并逐渐稳定。位移和延迟按材料、长度和动作强弱决定，不把整条布当刚性矩形同步平移，也不让它无原因地自行摆动。

逐帧关键姿势可以表现惯性。使用曲线、IK、接触约束或 `bake` 时，仍需设计动作和检查结果；工具求解通过不等于自然运动。只有实际运行相应物理解算时才称为物理模拟，手绘的拖拽和回摆应描述为逐帧跟随运动。

*Keep attachment points connected. Free ends lag acceleration, overshoot deceleration, and settle according to the material. Distinguish authored secondary motion from an actual physics simulation.*

## 交付前看什么

按实际帧时长播放完整动作，再对照准备、发力、命中和收势的关键帧。检查身体是否在传达预期力量、脚接触和握持是否成立、附属物是否跟随受力、恢复或循环衔接是否突跳。看不到播放效果时，标明时序未核，不把静态关键帧检查说成播放验收。

例如：普通跨步刺击若只有手臂和枪在动、下肢与躯干始终不变，应检查是否遗漏了动作设计需要的发力；有意的站桩、炮台或局部攻击不能仅因没有迈步就判错。图集校验与请求次数只检查结构和编辑效率，不能代替这项视觉检查。
