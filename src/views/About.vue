<template>
  <section class="about-view">
    <el-card class="panel">
      <div class="panel-title">Cloudreve Sync</div>
      <p class="about-text">
        本客户端不覆盖文件，不自动解决冲突。冲突发生时始终保留双方文件副本，
        所有原始时间戳通过 metadata 保存与恢复。
      </p>
      <div class="about-grid">
        <div>
          <div class="summary-label">版本</div>
          <div class="summary-value">0.1.0</div>
        </div>
        <div>
          <div class="summary-label">后端协议</div>
          <div class="summary-value">Cloudreve REST API</div>
        </div>
        <div>
          <div class="summary-label">同步策略</div>
          <div class="summary-value">双向 · 冲突双保留 · 软删除</div>
        </div>
      </div>
      <div class="about-actions">
        <el-button>查看诊断信息</el-button>
        <el-button type="primary">打开文档</el-button>
      </div>
    </el-card>
  </section>
</template>
