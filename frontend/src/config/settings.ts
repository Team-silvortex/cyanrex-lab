export const DOCS_LINK_STYLE = {
  textDecoration: "none",
  border: "1px solid #2f4f88",
  borderRadius: 10,
  padding: "10px 14px",
  background: "linear-gradient(130deg, #1f3f79, #2d63bf)",
  color: "#f3f7ff",
};

export const DOCS_QUICK_LINKS = [
  { href: "/learn/teacher-guide", titleKey: "settings.docsTeacherGuide" },
  { href: "/learn/student-guide", titleKey: "settings.docsStudentGuide" },
  { href: "/learn/concepts", titleKey: "settings.docsConcepts" },
  { href: "/learn/labs/01-first-program", titleKey: "settings.docsFirstLab" },
] as const;
