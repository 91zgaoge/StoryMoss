import { useState, useEffect } from 'react';
import { useAppStore } from '@/stores/appStore';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import {
  getContractTree, getRuntimeContract, getChapterCommits,
  createMasterSetting, createChapterContract, initChapterCommit,
  evaluateReadingPower, getReadingPowerTrend, getChaseDebts,
  buildMemoryPack, getMemoryItems, antiAiReview, auditStory, evolveStyleFromAntiAiReview,
  getGenreProfiles, checkProjectionHealth, saveGenreProfile, deleteGenreProfile,
  logFeatureUsage,
} from '@/services/tauri';
import type {
  ContractTree, RuntimeContract, ChapterCommit,
  ReadingPowerEvaluation, ChaseDebt, MemoryPack, MemoryItem,
  AntiAiReview, GenreProfile, StoryAnalysisReport,
} from '@/services/tauri';
import type { ProjectionHealthReport } from '@/types/v3';
import {
  FileText, TrendingUp, Brain, ShieldAlert, BookOpen,
  ChevronRight, Loader2, Zap, Activity, CheckCircle, AlertTriangle,
  Layers, Edit, Copy, Trash2, Plus, Radar,
} from 'lucide-react';
import { StyleDnaRadar } from '@/components/StyleDnaRadar';
import toast from 'react-hot-toast';
import { createLogger } from '@/utils/logger';

const styleLogger = createLogger('ui:StorySystem');

export function StorySystem() {
  const currentStory = useAppStore((s) => s.currentStory);
  const [activeTab, setActiveTab] = useState<'contracts' | 'commits' | 'reading' | 'memory' | 'audit' | 'anti-ai' | 'genres' | 'style-dna'>('contracts');
  const [isLoading, setIsLoading] = useState(false);

  // Contracts
  const [contractTree, setContractTree] = useState<ContractTree | null>(null);
  const [runtimeContract, setRuntimeContract] = useState<RuntimeContract | null>(null);
  const [selectedChapter, setSelectedChapter] = useState<number>(1);

  // Commits
  const [commits, setCommits] = useState<ChapterCommit[]>([]);

  // Reading Power
  const [readingTrend, setReadingTrend] = useState<ReadingPowerEvaluation[]>([]);
  const [chaseDebts, setChaseDebts] = useState<ChaseDebt[]>([]);
  const [isEvaluating, setIsEvaluating] = useState(false);

  // Memory
  const [memoryItems, setMemoryItems] = useState<MemoryItem[]>([]);

  // Anti-AI
  const [reviewText, setReviewText] = useState('');
  const [reviewResult, setReviewResult] = useState<AntiAiReview | null>(null);
  const [isReviewing, setIsReviewing] = useState(false);
  const [isEvolving, setIsEvolving] = useState(false);

  // Audit
  const [auditReport, setAuditReport] = useState<StoryAnalysisReport | null>(null);
  const [isAuditing, setIsAuditing] = useState(false);

  // Projection Health
  const [healthReports, setHealthReports] = useState<Record<number, ProjectionHealthReport>>({});
  const [checkingHealth, setCheckingHealth] = useState<Set<number>>(new Set());

  // Genre profiles
  const [genres, setGenres] = useState<GenreProfile[]>([]);
  const [editingGenre, setEditingGenre] = useState<GenreProfile | null>(null);
  const [showGenreForm, setShowGenreForm] = useState(false);
  const [showAntiAiCard, setShowAntiAiCard] = useState(false);

  useEffect(() => {
    loadGenres();
  }, []);

  useEffect(() => {
    if (currentStory?.id) {
      loadContracts();
      loadCommits();
      loadReadingPower();
      loadMemory();
    }
  }, [currentStory?.id]);

  // Feature usage telemetry
  useEffect(() => {
    if (!currentStory?.id) return;
    const featureMap: Record<string, string> = {
      contracts: 'story_contract',
      reading: 'reading_power',
      memory: 'memory_pack',
      audit: 'story_audit',
      'anti-ai': 'anti_ai_review',
      genres: 'genre_template',
    };
    const featureId = featureMap[activeTab];
    if (featureId) {
      logFeatureUsage(featureId, 'opened', currentStory.id);
    }
  }, [activeTab, currentStory?.id]);

  const loadGenres = async () => {
    try {
      const data = await getGenreProfiles();
      setGenres(data);
    } catch (e) {
      // silent fail
    }
  };

  const loadContracts = async () => {
    if (!currentStory) return;
    try {
      const tree = await getContractTree(currentStory.id);
      setContractTree(tree);
      const runtime = await getRuntimeContract(currentStory.id, selectedChapter);
      setRuntimeContract(runtime);
    } catch (e) {
      toast.error('加载合同失败');
    }
  };

  const loadCommits = async () => {
    if (!currentStory) return;
    try {
      const data = await getChapterCommits(currentStory.id);
      setCommits(data);
    } catch (e) {
      // silent fail
    }
  };

  const loadReadingPower = async () => {
    if (!currentStory) return;
    try {
      const [trend, debts] = await Promise.all([
        getReadingPowerTrend(currentStory.id, 10),
        getChaseDebts(currentStory.id),
      ]);
      setReadingTrend(trend);
      setChaseDebts(debts);
    } catch (e) {
      // silent fail
    }
  };

  const handleEvaluateReadingPower = async () => {
    if (!currentStory) return;
    setIsEvaluating(true);
    try {
      const result = await evaluateReadingPower(currentStory.id, selectedChapter);
      toast.success(`第${selectedChapter}章追读力评估完成: ${(result.score * 100).toFixed(0)}分`);
      loadReadingPower();
    } catch (e) {
      toast.error('评估失败');
    } finally {
      setIsEvaluating(false);
    }
  };

  const loadMemory = async () => {
    if (!currentStory) return;
    try {
      const data = await getMemoryItems(currentStory.id);
      setMemoryItems(data);
    } catch (e) {
      // silent fail
    }
  };

  const handleCheckHealth = async (chapterNumber: number) => {
    if (!currentStory) return;
    setCheckingHealth((prev) => new Set(prev).add(chapterNumber));
    try {
      const report = await checkProjectionHealth(currentStory.id, chapterNumber);
      setHealthReports((prev) => ({ ...prev, [chapterNumber]: report }));
    } catch (e) {
      toast.error(`健康检查失败: ${e}`);
    } finally {
      setCheckingHealth((prev) => {
        const next = new Set(prev);
        next.delete(chapterNumber);
        return next;
      });
    }
  };

  const handleAntiAiReview = async () => {
    if (!reviewText.trim()) {
      toast.error('请输入要审查的文本');
      return;
    }
    setIsReviewing(true);
    try {
      const result = await antiAiReview(reviewText, currentStory?.genre || undefined);
      setReviewResult(result);
      toast.success('审查完成');
      if (currentStory?.id) {
        logFeatureUsage('anti_ai_review', 'executed', currentStory.id);
      }
    } catch (e) {
      toast.error('审查失败');
    } finally {
      setIsReviewing(false);
    }
  };

  const handleAudit = async () => {
    if (!currentStory) return;
    setIsAuditing(true);
    try {
      const result = await auditStory(currentStory.id);
      setAuditReport(result);
      toast.success('审计完成');
    } catch (e) {
      toast.error('审计失败');
    } finally {
      setIsAuditing(false);
    }
  };

  const handleEvolveStyle = async () => {
    if (!currentStory || !reviewResult) return;
    setIsEvolving(true);
    try {
      const delta = await evolveStyleFromAntiAiReview(currentStory.id, reviewResult);
      if (delta.reasons.length > 0) {
        toast.success(`风格已进化: ${delta.reasons.length} 项调整`);
      } else {
        toast.success('风格无需调整');
      }
    } catch (e) {
      toast.error('风格进化失败');
    } finally {
      setIsEvolving(false);
    }
  };

  const tabs = [
    { id: 'contracts' as const, label: '合同', icon: FileText },
    { id: 'commits' as const, label: '提交链', icon: BookOpen },
    { id: 'reading' as const, label: '追读力', icon: TrendingUp },
    { id: 'memory' as const, label: '记忆', icon: Brain },
    { id: 'audit' as const, label: '审计', icon: ShieldAlert },
    { id: 'genres' as const, label: '体裁', icon: Layers },
    { id: 'style-dna' as const, label: '风格 DNA', icon: Radar },
  ];

  if (!currentStory) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">
        请先选择一个故事
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white mb-1">Story System</h1>
        <p className="text-gray-400 text-sm">{currentStory.title} — 合同驱动写作体系</p>
      </div>

      {/* Tabs */}
      <div className="flex gap-2 mb-6 border-b border-cinema-800 pb-2">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              activeTab === tab.id
                ? 'bg-cinema-gold/20 text-cinema-gold'
                : 'text-gray-400 hover:text-white hover:bg-cinema-800'
            }`}
          >
            <tab.icon className="w-4 h-4" />
            {tab.label}
          </button>
        ))}
      </div>

      {/* Contracts Tab */}
      {activeTab === 'contracts' && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <Card>
            <CardContent className="p-4">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                  <FileText className="w-5 h-5 text-cinema-gold" />
                  合同树
                </h3>
                {!contractTree?.master_setting && (
                  <Button
                    size="sm"
                    onClick={async () => {
                      if (!currentStory) return;
                      try {
                        await createMasterSetting({
                          story_id: currentStory.id,
                          genre: currentStory.genre || '小说',
                          core_tone: currentStory.tone || '中性',
                          pacing_strategy: '正常',
                          anti_patterns: [],
                          world_rules: [],
                        });
                        toast.success('世界观合同已创建');
                        loadContracts();
                      } catch (e) {
                        toast.error('创建世界观合同失败');
                      }
                    }}
                  >
                    <Plus className="w-4 h-4 mr-1" />
                    生成世界观合同
                  </Button>
                )}
              </div>
              {contractTree?.master_setting ? (
                <div className="space-y-3">
                  <div className="p-3 bg-cinema-800 rounded-lg">
                    <p className="text-sm text-gray-400">MASTER_SETTING</p>
                    <p className="text-white text-sm mt-1">{contractTree.master_setting.contract_json.slice(0, 200)}...</p>
                  </div>
                  <p className="text-sm text-gray-400">章节合同: {Object.keys(contractTree.chapters).length} 个</p>
                </div>
              ) : (
                <p className="text-gray-500 text-sm">暂无合同，请先创建 MASTER_SETTING</p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardContent className="p-4">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-semibold text-white">运行时合同</h3>
                <Button
                  size="sm"
                  onClick={async () => {
                    if (!currentStory) return;
                    try {
                      await createChapterContract({
                        story_id: currentStory.id,
                        chapter_number: selectedChapter,
                        goal: `完成第${selectedChapter}章的情节推进`,
                        must_cover_nodes: [],
                        forbidden_zones: [],
                      });
                      toast.success(`第${selectedChapter}章合同已创建`);
                      loadContracts();
                    } catch (e) {
                      toast.error('创建章节合同失败');
                    }
                  }}
                >
                  <Plus className="w-4 h-4 mr-1" />
                  生成章节合同
                </Button>
              </div>
              <div className="flex items-center gap-2 mb-4">
                <input
                  type="number"
                  value={selectedChapter}
                  onChange={(e) => setSelectedChapter(parseInt(e.target.value) || 1)}
                  className="bg-cinema-800 border border-cinema-700 rounded px-3 py-1 text-white text-sm w-20"
                  min={1}
                />
                <Button size="sm" onClick={loadContracts}>加载</Button>
              </div>
              {runtimeContract ? (
                <div className="space-y-2 text-sm">
                  <p className="text-gray-400">核心基调: {JSON.parse(runtimeContract.master_setting.contract_json).core_tone || 'N/A'}</p>
                  <p className="text-gray-400">体裁: {JSON.parse(runtimeContract.master_setting.contract_json).genre || 'N/A'}</p>
                  {runtimeContract.chapter_contract && (
                    <p className="text-gray-400">章节目标: {JSON.parse(runtimeContract.chapter_contract.contract_json).chapter_directive?.goal || 'N/A'}</p>
                  )}
                </div>
              ) : (
                <p className="text-gray-500 text-sm">点击加载查看运行时合同</p>
              )}
            </CardContent>
          </Card>
        </div>
      )}

      {/* Commits Tab */}
      {activeTab === 'commits' && (
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white">章节提交链</h3>
              {commits.length === 0 && (
                <Button
                  size="sm"
                  onClick={async () => {
                    if (!currentStory) return;
                    try {
                      await initChapterCommit(currentStory.id, selectedChapter);
                      toast.success(`第${selectedChapter}章提交已初始化`);
                      loadCommits();
                    } catch (e) {
                      toast.error('初始化提交失败');
                    }
                  }}
                >
                  <Plus className="w-4 h-4 mr-1" />
                  初始化提交
                </Button>
              )}
            </div>
            {commits.length === 0 ? (
              <p className="text-gray-500 text-sm">暂无提交记录</p>
            ) : (
              <div className="space-y-2">
                {commits.map((commit) => {
                  const report = healthReports[commit.chapter_number];
                  const isChecking = checkingHealth.has(commit.chapter_number);
                  return (
                    <div key={commit.id} className="p-3 bg-cinema-800 rounded-lg">
                      <div className="flex items-center justify-between">
                        <div>
                          <p className="text-white text-sm font-medium">第{commit.chapter_number}章</p>
                          <p className="text-gray-500 text-xs">状态: {commit.status}</p>
                        </div>
                        <div className="flex items-center gap-2">
                          {report && (
                            <span className={`text-xs px-2 py-0.5 rounded-full ${report.overall_healthy ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'}`}>
                              {report.overall_healthy ? '健康' : '异常'}
                            </span>
                          )}
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => handleCheckHealth(commit.chapter_number)}
                            disabled={isChecking}
                          >
                            {isChecking ? (
                              <Loader2 className="w-3 h-3 animate-spin" />
                            ) : (
                              <Activity className="w-3 h-3" />
                            )}
                            <span className="ml-1 text-xs">健康检查</span>
                          </Button>
                        </div>
                      </div>
                      {report && (
                        <div className="mt-2 grid grid-cols-5 gap-2">
                          {report.writers.map((w: { name: string; status: string }) => (
                            <div
                              key={w.name}
                              className={`text-[10px] px-1.5 py-0.5 rounded text-center ${
                                w.status === 'success' || w.status.startsWith('skipped')
                                  ? 'bg-green-500/10 text-green-400'
                                  : w.status === 'pending'
                                    ? 'bg-yellow-500/10 text-yellow-400'
                                    : 'bg-red-500/10 text-red-400'
                              }`}
                              title={w.status}
                            >
                              <span className="capitalize">{w.name}</span>
                              {w.status === 'success' || w.status.startsWith('skipped') ? (
                                <CheckCircle className="w-3 h-3 mx-auto mt-0.5" />
                              ) : w.status === 'pending' ? (
                                <Loader2 className="w-3 h-3 mx-auto mt-0.5 animate-spin" />
                              ) : (
                                <AlertTriangle className="w-3 h-3 mx-auto mt-0.5" />
                              )}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* Reading Power Tab */}
      {activeTab === 'reading' && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <Card>
            <CardContent className="p-4">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                  <TrendingUp className="w-5 h-5 text-green-400" />
                  追读力趋势
                </h3>
                <div className="flex items-center gap-2">
                  <select
                    value={selectedChapter}
                    onChange={(e) => setSelectedChapter(Number(e.target.value))}
                    className="bg-cinema-800 text-white text-sm rounded px-2 py-1 border border-cinema-700"
                  >
                    {Array.from({ length: 20 }, (_, i) => i + 1).map((n) => (
                      <option key={n} value={n}>第{n}章</option>
                    ))}
                  </select>
                  <Button
                    size="sm"
                    onClick={handleEvaluateReadingPower}
                    disabled={isEvaluating}
                  >
                    {isEvaluating ? (
                      <Loader2 className="w-4 h-4 animate-spin mr-1" />
                    ) : (
                      <Zap className="w-4 h-4 mr-1" />
                    )}
                    评估
                  </Button>
                </div>
              </div>
              {readingTrend.length === 0 ? (
                <p className="text-gray-500 text-sm">暂无数据</p>
              ) : (
                <div className="space-y-3">
                  {readingTrend.map((rp) => (
                    <div key={rp.chapter_number} className="flex items-center gap-3">
                      <span className="text-gray-400 text-sm w-12">Ch{rp.chapter_number}</span>
                      <div className="flex-1 h-6 bg-cinema-800 rounded-full overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all"
                          style={{
                            width: `${rp.score * 100}%`,
                            backgroundColor: rp.score > 0.7 ? '#4ade80' : rp.score > 0.4 ? '#fbbf24' : '#f87171',
                          }}
                        />
                      </div>
                      <span className="text-white text-sm w-10 text-right">{(rp.score * 100).toFixed(0)}</span>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardContent className="p-4">
              <h3 className="text-lg font-semibold text-white mb-4">追读债务</h3>
              {chaseDebts.length === 0 ? (
                <p className="text-gray-500 text-sm">无活跃债务</p>
              ) : (
                <div className="space-y-2">
                  {chaseDebts.map((debt) => (
                    <div key={debt.id} className="p-3 bg-cinema-800 rounded-lg">
                      <p className="text-white text-sm">{debt.debt_type} — 第{debt.source_chapter}章</p>
                      <p className="text-gray-500 text-xs">金额: {debt.current_amount} / 截止: 第{debt.due_chapter}章</p>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </div>
      )}

      {/* Memory Tab */}
      {activeTab === 'memory' && (
        <Card>
          <CardContent className="p-4">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                <Brain className="w-5 h-5 text-purple-400" />
                记忆项 ({memoryItems.length})
              </h3>
              <Button
                size="sm"
                onClick={async () => {
                  if (!currentStory) return;
                  try {
                    await buildMemoryPack(currentStory.id, selectedChapter, 'write');
                    toast.success('记忆包构建成功');
                    loadMemory();
                  } catch (e) {
                    toast.error('构建记忆包失败');
                  }
                }}
              >
                <Zap className="w-4 h-4 mr-1" />
                构建记忆包
              </Button>
            </div>
            {memoryItems.length === 0 ? (
              <p className="text-gray-500 text-sm">暂无记忆项</p>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                {memoryItems.slice(0, 30).map((item) => (
                  <div key={item.id} className="p-3 bg-cinema-800 rounded-lg">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs px-2 py-0.5 rounded bg-cinema-700 text-gray-300">{item.category}</span>
                      <span className="text-xs text-gray-500">Ch{item.source_chapter || '?'}</span>
                    </div>
                    <p className="text-white text-sm">{item.subject || item.value || '(空)'}</p>
                    <p className="text-gray-500 text-xs mt-1">置信度: {(item.confidence * 100).toFixed(0)}%</p>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* Audit Tab */}
      {activeTab === 'audit' && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold text-white flex items-center gap-2">
              <ShieldAlert className="w-5 h-5 text-orange-400" />
              叙事审计
            </h3>
            <Button
              size="sm"
              onClick={handleAudit}
              disabled={isAuditing}
            >
              {isAuditing ? (
                <Loader2 className="w-4 h-4 animate-spin mr-1" />
              ) : (
                <Activity className="w-4 h-4 mr-1" />
              )}
              运行全面审计
            </Button>
          </div>

          {auditReport ? (
            <div className="space-y-4">
              {/* Overall Score */}
              <Card>
                <CardContent className="p-4">
                  <div className="flex items-center gap-4">
                    <div className="text-4xl font-bold text-white">
                      {auditReport.overall_score}
                    </div>
                    <div>
                      <p className="text-white font-medium">
                        {auditReport.overall_score >= 80 ? '结构健康' : auditReport.overall_score >= 50 ? '需要关注' : '问题较多'}
                      </p>
                      <p className="text-gray-500 text-sm">
                        综合评分 (0-100)
                      </p>
                    </div>
                  </div>
                </CardContent>
              </Card>

              {/* Dimensions */}
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {auditReport.dimensions.map((dim) => (
                  <Card key={dim.name}>
                    <CardContent className="p-4">
                      <h4 className="text-white font-medium mb-2">{dim.name}</h4>
                      <div className="flex items-center gap-2 mb-2">
                        <div className="flex-1 h-3 bg-cinema-800 rounded-full overflow-hidden">
                          <div
                            className="h-full rounded-full transition-all"
                            style={{
                              width: `${dim.score}%`,
                              backgroundColor: dim.score > 70 ? '#4ade80' : dim.score > 40 ? '#fbbf24' : '#f87171',
                            }}
                          />
                        </div>
                        <span className="text-white text-sm w-8 text-right">{dim.score}</span>
                      </div>
                      <p className="text-gray-500 text-xs">{dim.description}</p>
                      {dim.details.length > 0 && (
                        <ul className="mt-2 space-y-1">
                          {dim.details.map((d, i) => (
                            <li key={i} className="text-gray-400 text-xs">{d}</li>
                          ))}
                        </ul>
                      )}
                    </CardContent>
                  </Card>
                ))}
              </div>

              {/* Recommendations */}
              {auditReport.recommendations.length > 0 && (
                <Card>
                  <CardContent className="p-4">
                    <h4 className="text-white font-medium mb-3">建议</h4>
                    <div className="space-y-2">
                      {auditReport.recommendations.map((rec, i) => (
                        <div key={i} className="p-2 bg-cinema-800 rounded text-gray-300 text-sm">{rec}</div>
                      ))}
                    </div>
                  </CardContent>
                </Card>
              )}
            </div>
          ) : (
            <Card>
              <CardContent className="p-4">
                <p className="text-gray-500 text-sm">
                  点击"运行全面审计"开始分析故事结构健康度
                </p>
              </CardContent>
            </Card>
          )}
        </div>
      )}

      {/* Genres Tab */}
      {activeTab === 'genres' && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold text-white flex items-center gap-2">
              <Layers className="w-5 h-5 text-cinema-gold" />
              体裁模板
            </h3>
            <Button size="sm" onClick={() => { setEditingGenre(null); setShowGenreForm(true); }}>
              <Plus className="w-3 h-3 mr-1" />
              新建
            </Button>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {genres.map((g) => (
              <Card key={g.id} className="relative">
                <CardContent className="p-4">
                  <div className="flex items-start justify-between">
                    <div>
                      <p className="text-white font-medium">{g.genre_name}</p>
                      <p className="text-gray-500 text-xs">{g.canonical_name}</p>
                      {g.is_builtin && (
                        <span className="inline-block mt-1 text-[10px] px-1.5 py-0.5 rounded bg-slate-700 text-slate-300">内置</span>
                      )}
                    </div>
                    <div className="flex items-center gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => { setEditingGenre(g); setShowGenreForm(true); }}
                        title="编辑"
                      >
                        <Edit className="w-3 h-3" />
                      </Button>
                      {!g.is_builtin && (
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={async () => {
                            if (!window.confirm('确认删除此体裁模板？')) return;
                            try {
                              await deleteGenreProfile(String(g.id));
                              toast.success('已删除');
                              loadGenres();
                            } catch (e) {
                              toast.error('删除失败');
                            }
                          }}
                          title="删除"
                        >
                          <Trash2 className="w-3 h-3 text-red-400" />
                        </Button>
                      )}
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setEditingGenre({ ...g, id: 0, genre_name: g.genre_name + ' (副本)', is_builtin: false } as any);
                          setShowGenreForm(true);
                        }}
                        title="复制为新模板"
                      >
                        <Copy className="w-3 h-3" />
                      </Button>
                    </div>
                  </div>
                  {g.core_tone && (
                    <p className="text-gray-400 text-xs mt-2 line-clamp-2">{g.core_tone}</p>
                  )}
                </CardContent>
              </Card>
            ))}
          </div>

          {showGenreForm && (
            <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
              <div className="bg-cinema-900 border border-cinema-700 rounded-lg p-6 w-full max-w-lg">
                <h3 className="text-white font-semibold mb-4">
                  {editingGenre?.id ? '编辑体裁' : '新建体裁'}
                </h3>
                <GenreForm
                  initial={editingGenre}
                  onSave={async (data) => {
                    try {
                      await saveGenreProfile(data as any);
                      toast.success('保存成功');
                      setShowGenreForm(false);
                      loadGenres();
                    } catch (e) {
                      toast.error('保存失败');
                    }
                  }}
                  onCancel={() => setShowGenreForm(false)}
                />
              </div>
            </div>
          )}
        </div>
      )}

      {/* Style DNA Tab */}
      {activeTab === 'style-dna' && (
        <div className="max-w-2xl">
          <StyleDnaRadar
            storyId={currentStory.id}
            onConstraintChange={(constraints) => {
              toast.success('StyleDNA 约束已更新');
              styleLogger.info('StyleDNA constraints updated', { constraints });
            }}
          />
        </div>
      )}

      {/* Anti-AI 可展开卡片 */}
      <div className="mt-8">
        <button
          onClick={() => {
            const next = !showAntiAiCard;
            setShowAntiAiCard(next);
            if (next && currentStory?.id) {
              logFeatureUsage('anti_ai_review', 'opened', currentStory.id);
            }
          }}
          className="flex items-center gap-2 text-sm text-gray-400 hover:text-white transition-colors"
        >
          <ShieldAlert className="w-4 h-4" />
          <span>Anti-AI 审查</span>
          <span className="text-xs text-gray-600 ml-1">{showAntiAiCard ? '收起' : '展开'}</span>
        </button>
        {showAntiAiCard && (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mt-3">
            <Card>
              <CardContent className="p-4">
                <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
                  <ShieldAlert className="w-5 h-5 text-red-400" />
                  Anti-AI 审查
                </h3>
                <textarea
                  value={reviewText}
                  onChange={(e) => setReviewText(e.target.value)}
                  placeholder="粘贴要审查的文本..."
                  className="w-full h-48 bg-cinema-800 border border-cinema-700 rounded-lg p-3 text-white text-sm resize-none focus:outline-none focus:border-cinema-gold"
                />
                <div className="flex justify-end mt-3">
                  <Button
                    onClick={handleAntiAiReview}
                    disabled={isReviewing}
                    className="flex items-center gap-2"
                  >
                    {isReviewing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Zap className="w-4 h-4" />}
                    开始审查
                  </Button>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardContent className="p-4">
                <h3 className="text-lg font-semibold text-white mb-4">审查结果</h3>
                {!reviewResult ? (
                  <p className="text-gray-500 text-sm">输入文本并点击审查</p>
                ) : (
                  <div className="space-y-4">
                    <div className="flex items-center gap-4">
                      <div className="text-3xl font-bold"
                        style={{
                          color: reviewResult.overall_score > 0.7 ? '#4ade80' : reviewResult.overall_score > 0.4 ? '#fbbf24' : '#f87171'
                        }}
                      >
                        {(reviewResult.overall_score * 100).toFixed(0)}
                      </div>
                      <div className="text-gray-400 text-sm">综合评分</div>
                    </div>

                    <div className="space-y-2">
                      {reviewResult.dimensions.map((dim) => (
                        <div key={dim.name} className="flex items-center gap-3">
                          <span className="text-gray-400 text-sm w-12">{dim.name}</span>
                          <div className="flex-1 h-4 bg-cinema-800 rounded-full overflow-hidden">
                            <div
                              className="h-full rounded-full"
                              style={{
                                width: `${dim.score * 100}%`,
                                backgroundColor: dim.score > 0.7 ? '#4ade80' : dim.score > 0.4 ? '#fbbf24' : '#f87171',
                              }}
                            />
                          </div>
                          <span className="text-white text-xs w-8">{(dim.score * 100).toFixed(0)}</span>
                        </div>
                      ))}
                    </div>

                    {reviewResult.issues.length > 0 && (
                      <div className="mt-4">
                        <h4 className="text-white text-sm font-medium mb-2">发现的问题</h4>
                        <div className="space-y-2">
                          {reviewResult.issues.slice(0, 5).map((issue, idx) => (
                            <div key={idx} className="p-2 bg-cinema-800 rounded text-sm">
                              <div className="flex items-center gap-2 mb-1">
                                <span className={`text-xs px-1.5 py-0.5 rounded ${
                                  issue.severity === 'high' ? 'bg-red-900/50 text-red-300' :
                                  issue.severity === 'medium' ? 'bg-yellow-900/50 text-yellow-300' :
                                  'bg-blue-900/50 text-blue-300'
                                }`}>
                                  {issue.severity}
                                </span>
                                <span className="text-gray-300">{issue.dimension}</span>
                              </div>
                              <p className="text-gray-400">{issue.description}</p>
                              <p className="text-cinema-gold text-xs mt-1">建议: {issue.suggestion}</p>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    <div className="mt-4">
                      <Button
                        size="sm"
                        onClick={handleEvolveStyle}
                        disabled={isEvolving}
                      >
                        {isEvolving ? (
                          <Loader2 className="w-4 h-4 animate-spin mr-1" />
                        ) : (
                          <Zap className="w-4 h-4 mr-1" />
                        )}
                        接受审校并进化风格
                      </Button>
                    </div>
                  </div>
                )}
              </CardContent>
            </Card>
          </div>
        )}
      </div>
    </div>
  );
}

// ==================== Genre Form Component ====================

interface GenreFormProps {
  initial: Partial<GenreProfile> | null;
  onSave: (data: Partial<GenreProfile>) => void;
  onCancel: () => void;
}

function GenreForm({ initial, onSave, onCancel }: GenreFormProps) {
  const [form, setForm] = useState({
    id: initial?.id ? String(initial.id) : undefined,
    genre_name: initial?.genre_name || '',
    canonical_name: initial?.canonical_name || '',
    core_tone: initial?.core_tone || '',
    pacing_strategy: initial?.pacing_strategy || '',
    anti_patterns_json: initial?.anti_patterns
      ? JSON.stringify(initial.anti_patterns)
      : '[]',
    reference_tables_json: initial?.reference_tables
      ? JSON.stringify(initial.reference_tables)
      : '',
  });

  const handleChange = (field: string, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  return (
    <div className="space-y-3">
      <div>
        <label className="text-gray-400 text-xs">体裁名称</label>
        <input
          className="w-full bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1"
          value={form.genre_name}
          onChange={(e) => handleChange('genre_name', e.target.value)}
          placeholder="如：修仙"
        />
      </div>
      <div>
        <label className="text-gray-400 text-xs">英文名</label>
        <input
          className="w-full bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1"
          value={form.canonical_name}
          onChange={(e) => handleChange('canonical_name', e.target.value)}
          placeholder="如：Cultivation"
        />
      </div>
      <div>
        <label className="text-gray-400 text-xs">核心基调</label>
        <textarea
          className="w-full h-20 bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1 resize-none"
          value={form.core_tone}
          onChange={(e) => handleChange('core_tone', e.target.value)}
          placeholder="描述该体裁的核心特征..."
        />
      </div>
      <div>
        <label className="text-gray-400 text-xs">节奏策略</label>
        <textarea
          className="w-full h-20 bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1 resize-none"
          value={form.pacing_strategy}
          onChange={(e) => handleChange('pacing_strategy', e.target.value)}
          placeholder="描述该体裁的节奏策略..."
        />
      </div>
      <div className="flex justify-end gap-2 pt-2">
        <Button size="sm" variant="ghost" onClick={onCancel}>取消</Button>
        <Button
          size="sm"
          onClick={() => onSave(form as any)}
          disabled={!form.genre_name || !form.canonical_name}
        >
          保存
        </Button>
      </div>
    </div>
  );
}
