param location string = resourceGroup().location
param envName string

resource log 'Microsoft.OperationalInsights/workspaces@2022-10-01' = {
  name: '${envName}-log'
  location: location
  properties: {
    sku: {
      name: 'PerGB2018'
    }
  }
}

resource cae 'Microsoft.App/managedEnvironments@2023-05-01' = {
  name: '${envName}-cae'
  location: location
  properties: {
    appLogsConfiguration: {
      destination: 'log-analytics'
      logAnalyticsConfiguration: {
        customerId: log.properties.customerId
        sharedKey: listKeys(log.id, log.apiVersion).primarySharedKey
      }
    }
  }
}

output containerAppsEnvironmentId string = cae.id
