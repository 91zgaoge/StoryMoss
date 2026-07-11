import { useEffect, useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { User, LogOut, BookOpen, ArrowLeft, Shield } from 'lucide-react'
import axios from 'axios'

const API_BASE = import.meta.env.VITE_API_URL || '/api'

interface UserInfo {
  id: string
  email?: string
  display_name?: string
  avatar_url?: string
}

export default function DashboardPage() {
  const navigate = useNavigate()
  const [user, setUser] = useState<UserInfo | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    const token = localStorage.getItem('sf_token')
    if (!token) {
      navigate('/login')
      return
    }

    // Fetch user info
    axios
      .get(`${API_BASE}/auth/me`, {
        headers: { Authorization: `Bearer ${token}` },
      })
      .then((res) => {
        setUser(res.data)
      })
      .catch(() => {
        localStorage.removeItem('sf_token')
        navigate('/login')
      })
      .finally(() => {
        setIsLoading(false)
      })
  }, [navigate])

  const handleLogout = () => {
    localStorage.removeItem('sf_token')
    navigate('/')
  }

  if (isLoading) {
    return (
      <div className="min-h-screen bg-cinema-950 flex items-center justify-center">
        <div className="text-gray-400">加载中...</div>
      </div>
    )
  }

  if (!user) return null

  return (
    <div className="min-h-screen bg-cinema-950">
      {/* Header */}
      <header className="border-b border-cinema-800/50 bg-cinema-900/50">
        <div className="max-w-6xl mx-auto px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link
              to="/"
              className="flex items-center gap-2 text-gray-400 hover:text-white transition-colors"
            >
              <ArrowLeft className="w-4 h-4" />
              <BookOpen className="w-5 h-5" />
              <span className="font-display font-bold text-white">草苔</span>
            </Link>
          </div>
          <button
            onClick={handleLogout}
            className="flex items-center gap-2 px-4 py-2 text-sm text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
          >
            <LogOut className="w-4 h-4" />
            退出登录
          </button>
        </div>
      </header>

      {/* Content */}
      <main className="max-w-6xl mx-auto px-6 py-12">
        <div className="grid md:grid-cols-3 gap-6">
          {/* User Card */}
          <div className="md:col-span-1">
            <div className="bg-cinema-900/50 border border-cinema-800/50 rounded-2xl p-6">
              <div className="flex flex-col items-center text-center">
                <div className="w-20 h-20 rounded-full bg-cinema-800 flex items-center justify-center mb-4">
                  {user.avatar_url ? (
                    <img
                      src={user.avatar_url}
                      alt=""
                      className="w-20 h-20 rounded-full object-cover"
                    />
                  ) : (
                    <User className="w-10 h-10 text-gray-400" />
                  )}
                </div>
                <h2 className="text-lg font-semibold text-white">
                  {user.display_name || '用户'}
                </h2>
                <p className="text-sm text-gray-400 mt-1">{user.email || ''}</p>
                <div className="mt-3 flex items-center gap-1 text-xs text-green-400">
                  <Shield className="w-3 h-3" />
                  <span>已认证</span>
                </div>
              </div>
            </div>
          </div>

          {/* Main Area */}
          <div className="md:col-span-2">
            <div className="bg-cinema-900/50 border border-cinema-800/50 rounded-2xl p-6">
              <h3 className="text-lg font-semibold text-white mb-4">
                欢迎使用 StoryMoss
              </h3>
              <p className="text-gray-400 text-sm leading-relaxed mb-6">
                云同步功能正在开发中。目前您可以通过桌面端应用进行创作，
                未来登录账号后将支持跨设备同步故事数据。
              </p>

              <div className="space-y-3">
                <div className="p-4 rounded-xl bg-cinema-800/30 border border-cinema-700/30">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-sm font-medium text-white">下载桌面端</p>
                      <p className="text-xs text-gray-500 mt-1">
                        获取完整的 AI 辅助创作体验
                      </p>
                    </div>
                    <button className="px-4 py-2 bg-cinema-gold text-cinema-900 rounded-lg text-sm font-medium hover:bg-cinema-gold-light transition-colors">
                      下载
                    </button>
                  </div>
                </div>

                <div className="p-4 rounded-xl bg-cinema-800/30 border border-cinema-700/30">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-sm font-medium text-white">GitHub</p>
                      <p className="text-xs text-gray-500 mt-1">
                        关注项目最新动态
                      </p>
                    </div>
                    <a
                      href="https://github.com/91zgaoge/StoryMoss"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="px-4 py-2 bg-cinema-800 text-white rounded-lg text-sm font-medium hover:bg-cinema-700 transition-colors"
                    >
                      访问
                    </a>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>
  )
}
