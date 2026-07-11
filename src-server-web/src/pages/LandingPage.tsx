import { Link } from 'react-router-dom'
import { 
  BookOpen, Sparkles, Brain, Palette, 
  ArrowRight, Download, Github, ChevronRight 
} from 'lucide-react'

export default function LandingPage() {
  return (
    <div className="min-h-screen bg-cinema-950">
      {/* Navbar */}
      <nav className="fixed top-0 left-0 right-0 z-50 bg-cinema-950/80 backdrop-blur-md border-b border-cinema-800/50">
        <div className="max-w-7xl mx-auto px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-cinema-gold to-cinema-gold-dark flex items-center justify-center">
              <BookOpen className="w-5 h-5 text-cinema-900" />
            </div>
            <span className="font-display text-xl font-bold text-white">草苔</span>
            <span className="text-xs text-gray-500 hidden sm:inline">StoryMoss</span>
          </div>
          <div className="flex items-center gap-4">
            <a 
              href="https://github.com/91zgaoge/StoryMoss" 
              target="_blank" 
              rel="noopener noreferrer"
              className="p-2 text-gray-400 hover:text-white transition-colors"
            >
              <Github className="w-5 h-5" />
            </a>
            <Link
              to="/login"
              className="px-4 py-2 bg-cinema-gold text-cinema-900 rounded-lg text-sm font-medium hover:bg-cinema-gold-light transition-colors"
            >
              登录
            </Link>
          </div>
        </div>
      </nav>

      {/* Hero */}
      <section className="pt-32 pb-20 px-6">
        <div className="max-w-5xl mx-auto text-center">
          <h1 className="font-display text-5xl md:text-7xl font-bold text-white leading-tight mb-6">
            让 AI 成为你的
            <span className="text-cinema-gold"> 创作伙伴</span>
          </h1>
          <p className="text-xl text-gray-400 max-w-2xl mx-auto mb-10 leading-relaxed">
            StoryMoss (草苔) — 越写越懂的 AI 辅助小说创作平台。
            从灵感构思到完稿出版，AI 深度理解你的故事世界。
          </p>
          <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
            <Link
              to="/login"
              className="px-8 py-3.5 bg-cinema-gold text-cinema-900 rounded-xl text-base font-semibold hover:bg-cinema-gold-light transition-colors flex items-center gap-2"
            >
              免费开始使用
              <ArrowRight className="w-5 h-5" />
            </Link>
            <button className="px-8 py-3.5 bg-cinema-800 text-white rounded-xl text-base font-medium hover:bg-cinema-700 transition-colors flex items-center gap-2">
              <Download className="w-5 h-5" />
              下载桌面端
            </button>
          </div>
        </div>
      </section>

      {/* Features */}
      <section className="py-20 px-6 border-t border-cinema-800/50">
        <div className="max-w-6xl mx-auto">
          <h2 className="font-display text-3xl font-bold text-white text-center mb-4">
            为小说创作者打造的完整工具链
          </h2>
          <p className="text-gray-400 text-center mb-16 max-w-xl mx-auto">
            从世界观构建到风格控制，从知识图谱到伏笔追踪，一切尽在掌握。
          </p>

          <div className="grid md:grid-cols-3 gap-6">
            <FeatureCard
              icon={<Sparkles className="w-6 h-6 text-cinema-gold" />}
              title="AI 智能续写"
              description="理解角色关系、叙事节奏和伏笔状态，生成连贯的剧情内容。支持多种写作风格 DNA。"
            />
            <FeatureCard
              icon={<Brain className="w-6 h-6 text-cinema-gold" />}
              title="知识图谱"
              description="自动提取人物、地点、事件构建知识图谱，确保跨章节设定一致性。"
            />
            <FeatureCard
              icon={<Palette className="w-6 h-6 text-cinema-gold" />}
              title="风格 DNA"
              description="海明威的简洁、普鲁斯特的意识流、马尔克斯的魔幻现实 — 一键切换大师文风。"
            />
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="py-20 px-6 border-t border-cinema-800/50">
        <div className="max-w-4xl mx-auto text-center">
          <h2 className="font-display text-3xl font-bold text-white mb-4">
            准备好开始你的创作之旅了吗？
          </h2>
          <p className="text-gray-400 mb-8">
            立即注册，免费体验完整的 AI 辅助创作流程。
          </p>
          <Link
            to="/login"
            className="inline-flex items-center gap-2 px-8 py-3.5 bg-cinema-gold text-cinema-900 rounded-xl text-base font-semibold hover:bg-cinema-gold-light transition-colors"
          >
            立即开始
            <ChevronRight className="w-5 h-5" />
          </Link>
        </div>
      </section>

      {/* Footer */}
      <footer className="py-8 px-6 border-t border-cinema-800/50">
        <div className="max-w-6xl mx-auto flex flex-col md:flex-row items-center justify-between gap-4">
          <div className="flex items-center gap-2 text-gray-500 text-sm">
            <BookOpen className="w-4 h-4" />
            <span>StoryMoss (草苔) v4.5.0</span>
          </div>
          <div className="flex items-center gap-6 text-sm text-gray-500">
            <a href="https://github.com/91zgaoge/StoryMoss" className="hover:text-gray-300 transition-colors">
              GitHub
            </a>
            <span>开源 · 免费 · 本地优先</span>
          </div>
        </div>
      </footer>
    </div>
  )
}

function FeatureCard({ icon, title, description }: { icon: React.ReactNode; title: string; description: string }) {
  return (
    <div className="p-6 rounded-2xl bg-cinema-900/50 border border-cinema-800/50 hover:border-cinema-gold/30 transition-colors">
      <div className="w-12 h-12 rounded-xl bg-cinema-800/50 flex items-center justify-center mb-4">
        {icon}
      </div>
      <h3 className="text-lg font-semibold text-white mb-2">{title}</h3>
      <p className="text-sm text-gray-400 leading-relaxed">{description}</p>
    </div>
  )
}
