import Link from "next/link";

import SidebarLayout from "../../src/components/SidebarLayout";

const sections = [
  { href: "/learn/teacher-guide", title: "教师快速开始", detail: "课堂拓扑、课时安排、验收与清理流程" },
  { href: "/learn/student-guide", title: "学生快速开始", detail: "登录、界面、编辑器能力和实验纪律" },
  { href: "/learn/concepts", title: "eBPF 知识地图", detail: "Hook、Context、Helper、Map、Verifier 与 BTF" },
  { href: "/learn/labs/01-first-program", title: "实验 1 · 第一个程序", detail: "XDP Pass、编译加载链路与安全卸载" },
  { href: "/learn/labs/02-trace-execve", title: "实验 2 · Tracepoint", detail: "观察 execve 与 bpf_printk" },
  { href: "/learn/labs/03-map-counter", title: "实验 3 · Map 计数", detail: "Per-CPU Array、状态和采样" },
  { href: "/learn/labs/04-ring-buffer", title: "实验 4 · Ring Buffer", detail: "结构化事件和用户态传输" },
  { href: "/learn/labs/05-verifier-debugging", title: "实验 5 · Verifier", detail: "空指针、边界、循环与日志调试" },
  { href: "/learn/troubleshooting", title: "故障排查", detail: "从页面、编译、加载到事件流逐层定位" },
  { href: "/learn/security", title: "安全与部署", detail: "个人实验、虚拟机隔离和课堂安全边界" },
];

export default function LearnIndexPage() {
  return (
    <SidebarLayout title="学习中心">
      <section className="panel">
        <p className="brand-kicker">CYANREX COURSE</p>
        <h2 style={{ marginTop: 4 }}>eBPF 学习中心</h2>
        <p className="meta">从第一个 XDP 程序开始，逐步掌握 Tracepoint、Map、Ring Buffer 和 Verifier。</p>
        <div className="grid cols-2" style={{ marginTop: 16 }}>
          {sections.map((section) => (
            <Link key={section.href} href={section.href} className="panel" style={{ display: "block", textDecoration: "none", background: "#0b1425" }}>
              <strong>{section.title}</strong>
              <p className="meta" style={{ marginBottom: 0 }}>{section.detail}</p>
            </Link>
          ))}
        </div>
      </section>
    </SidebarLayout>
  );
}
