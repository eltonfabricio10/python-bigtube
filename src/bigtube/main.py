#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Ponto de entrada principal para o aplicativo BigTube

Este módulo serve como o ponto de entrada principal para o aplicativo BigTube.
Ele verifica as dependências necessárias, inicializa os componentes principais
e inicia a aplicação.
"""
import sys
import logging
import gi
from bigtube.app import BigTubeApp
from bigtube.utils import check_dependencies

# Configuração do logger
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

logger = logging.getLogger(__name__)

def main() -> int:
    """
    Função principal para iniciar o aplicativo.
    
    Verifica as dependências, inicializa os componentes necessários
    e inicia o aplicativo.
    
    Returns:
        int: Código de saída do aplicativo
    """
    # Verifica dependências
    try:
        check_dependencies()
    except ImportError as e:
        logger.error(f"Erro de dependência: {e}")
        return 1
    
    # Inicializa GStreamer
    gi.require_version('Gst', '1.0')
    from gi.repository import Gst
    Gst.init(None)
    
    # Inicia o aplicativo
    app = BigTubeApp()
    return app.run(sys.argv)

if __name__ == "__main__":
    sys.exit(main())