import { useState, useEffect } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { Chrome, Github, ArrowLeft, Loader2 } from 'lucide-react'
import axios from 'axios'

const API_BASE = import.meta.env.VITE_API_URL || '/api'

export default function LoginPage() {
  const navigate = useNavigate()
  const [config, setConfig] = useState({
    google_enabled: false,
    github_enabled: false,
    wechat_enabled: false,
    qq_enabled: false,
  })
  const [isLoading, setIsLoading] = useState<string | null>(null)

  useEffect(() => {
    // Load auth config
    axios.get(`${API_BASE}/auth/config`).then(res => {
      setConfig(res.data)
    }).catch(() => {
      // Fallback: try to enable all for dev
      setConfig({
        google_enabled: true,
        github_enabled: true,
        wechat_enabled: false,
        qq_enabled: false,
      })
    })
  }, [])

  const handleOAuthLogin = async (provider: string) => {
    setIsLoading(provider)
    try {
      const res = await axios.get(`${API_BASE}/auth/${provider}/start`)
      const { auth_url } = res.data
      // Redirect to OAuth provider
      window.location.href = auth_url
    } catch (error) {
      console.error('OAuth start failed:', error)
      alert('登录启动失败，请检查服务端配置')
    } finally {
      setIsLoading(null)
    }
  }

  return (
    <div className="min-h-screen bg-cinema-950 flex items-center justify-center px-6">
      <div className="w-full max-w-md">
        {/* Back button */}
        <Link
          to="/"
          className="inline-flex items-center gap-2 text-gray-400 hover:text-white transition-colors mb-8"
        >
          <ArrowLeft className="w-4 h-4" />
          <span className="text-sm">返回首页</span>
        </Link>

        {/* Card */}
        <div className="bg-cinema-900/50 border border-cinema-800/50 rounded-2xl p-8">
          <div className="text-center mb-8">
            <h1 className="font-display text-2xl font-bold text-white mb-2">
              登录 StoryMoss
            </h1>
            <p className="text-sm text-gray-400">
              选择以下方式登录或注册
            </p>
          </div>

          <div className="space-y-3">
            {config.google_enabled && (
              <button
                onClick={() => handleOAuthLogin('google')}
                disabled={isLoading !== null}
                className="w-full flex items-center justify-center gap-3 px-4 py-3 bg-cinema-800/50 border border-cinema-700/50 rounded-xl text-white hover:bg-cinema-800 transition-colors disabled:opacity-50"
              >
                {isLoading === 'google' ? (
                  <Loader2 className="w-5 h-5 animate-spin" />
                ) : (
                  <Chrome className="w-5 h-5 text-blue-400" />
                )}
                <span className="text-sm font-medium">使用 Google 登录</span>
              </button>
            )}

            {config.github_enabled && (
              <button
                onClick={() => handleOAuthLogin('github')}
                disabled={isLoading !== null}
                className="w-full flex items-center justify-center gap-3 px-4 py-3 bg-cinema-800/50 border border-cinema-700/50 rounded-xl text-white hover:bg-cinema-800 transition-colors disabled:opacity-50"
              >
                {isLoading === 'github' ? (
                  <Loader2 className="w-5 h-5 animate-spin" />
                ) : (
                  <Github className="w-5 h-5" />
                )}
                <span className="text-sm font-medium">使用 GitHub 登录</span>
              </button>
            )}

            {!config.google_enabled && !config.github_enabled && !config.wechat_enabled && !config.qq_enabled && (
              <div className="text-center py-8">
                <p className="text-sm text-gray-500">尚未配置 OAuth 登录选项</p>
                <p className="text-xs text-gray-600 mt-1">
                  请配置环境变量后重启服务端
                </p>
              </div>
            )}
          </div>

          <div className="mt-6 pt-6 border-t border-cinema-800/50">
            <p className="text-xs text-gray-500 text-center">
              登录即表示您同意服务条款和隐私政策
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}
